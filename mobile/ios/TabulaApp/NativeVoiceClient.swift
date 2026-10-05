import AVFAudio
import Foundation
import LiveKit
import TabulaShared
import UIKit

/// Native LiveKit media plane. Kotlin owns session policy; no credential reaches the game (ADR-0037).
/// Protocol methods and mutable adapter state are confined to the host UI thread.
final class NativeVoiceClient: NSObject, VoiceClient {
    private var current: VoiceSession?
    private var cleanupTail: Task<Void, Never>?
    private var cleanupFailed = false
    private var closed = false
    private var notificationTokens: [NSObjectProtocol] = []

    override init() {
        // Provider diagnostics may contain room/participant/endpoint information. They are never
        // a public error and are disabled before the first SDK Room is constructed.
        LiveKitSDK.disableLogging()
        super.init()
        let center = NotificationCenter.default
        notificationTokens.append(center.addObserver(
            forName: AVAudioSession.interruptionNotification, object: nil, queue: .main
        ) { [weak self] notification in
            let raw = (notification.userInfo?[AVAudioSessionInterruptionTypeKey] as? NSNumber)?.uintValue
            if raw == AVAudioSession.InterruptionType.began.rawValue { self?.audioInterrupted() }
        })
        notificationTokens.append(center.addObserver(
            forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main
        ) { [weak self] notification in
            let raw = (notification.userInfo?[AVAudioSessionRouteChangeReasonKey] as? NSNumber)?.uintValue
            // Removing a headset must not unexpectedly move a private conversation to the speaker.
            if raw == AVAudioSession.RouteChangeReason.oldDeviceUnavailable.rawValue ||
                raw == AVAudioSession.RouteChangeReason.noSuitableRouteForCategory.rawValue {
                self?.audioInterrupted()
            }
        })
        for name in [AVAudioSession.mediaServicesWereLostNotification, AVAudioSession.mediaServicesWereResetNotification] {
            notificationTokens.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                self?.audioInterrupted()
            })
        }
    }

    func connect(attempt: Int64, grant: VoiceJoinGrant, observer: VoiceClientObserver) {
        precondition(Thread.isMainThread)
        disconnect()
        guard !closed else { return }
        guard !cleanupFailed else {
            observer.onConnection(attempt: attempt, connection: .failed, error: VoiceError.publicationFailed)
            return
        }
        guard grant.expiresAtEpochSeconds > Int64(Date().timeIntervalSince1970) else {
            observer.onConnection(attempt: attempt, connection: .failed, error: VoiceError.grantExpired)
            return
        }
        let session = VoiceSession(owner: self, attempt: attempt, grant: grant, observer: observer)
        current = session
        observer.onConnection(attempt: attempt, connection: .connecting, error: nil)
        let previousCleanup = cleanupTail
        // Read-only join first. Microphone capture is possible only from an explicit later command.
        session.joinTask = Task { @MainActor [weak self, session] in
            // LiveKit's AudioManager is shared. A new Room must not race retired capture teardown.
            await previousCleanup?.value
            guard self?.isCurrent(session) == true, !Task.isCancelled else { await session.room.disconnect(); return }
            guard self?.cleanupFailed == false else {
                self?.reportConnection(session, .failed, error: VoiceError.publicationFailed)
                return
            }
            guard session.expiresAt > Int64(Date().timeIntervalSince1970) else {
                self?.reportConnection(session, .failed, error: VoiceError.grantExpired)
                return
            }
            do {
                try await session.room.connect(
                    url: grant.endpoint, token: grant.token,
                    connectOptions: ConnectOptions(autoSubscribe: true, enableMicrophone: false)
                )
                guard self?.isCurrent(session) == true, !Task.isCancelled else {
                    await session.room.disconnect()
                    return
                }
                self?.reportConnection(session, .connected)
            } catch {
                if self?.isCurrent(session) == true {
                    self?.reportConnection(session, .failed, error: VoiceError.connectionFailed)
                }
                // A cancelled or late connect can still finish provider work. Always tear its room down.
                await session.room.disconnect()
            }
            session.joinTask = nil
        }
    }

    func setMicrophoneEnabled(attempt: Int64, command: Int64, enabled: Bool) {
        precondition(Thread.isMainThread)
        guard let session = current, session.attempt == attempt, !closed else { return }
        session.microphoneCommand = command
        let previous = session.microphoneTask
        previous?.cancel()
        session.microphoneTask = Task { @MainActor [weak self, session] in
            // Do not let a slow permission or publication completion overtake a newer mic command.
            await previous?.value
            guard self?.isCurrent(session, command: command) == true, !Task.isCancelled else { return }
            guard session.expiresAt > Int64(Date().timeIntervalSince1970) else {
                self?.reportMicrophone(session, command: command, error: VoiceError.grantExpired)
                return
            }
            if enabled {
                guard session.canPublishMicrophone else {
                    self?.reportMicrophone(session, command: command, error: VoiceError.publicationFailed)
                    return
                }
                let granted = await Self.requestMicrophonePermission()
                guard self?.isCurrent(session, command: command) == true, !Task.isCancelled else { return }
                guard granted else {
                    self?.reportMicrophone(session, command: command, error: VoiceError.permissionDenied)
                    return
                }
            }
            guard session.expiresAt > Int64(Date().timeIntervalSince1970) else {
                self?.reportMicrophone(session, command: command, error: VoiceError.grantExpired)
                return
            }
            if enabled && !session.canPublishMicrophone {
                self?.reportMicrophone(session, command: command, error: VoiceError.publicationFailed)
                return
            }
            guard session.room.connectionState == .connected else {
                self?.reportMicrophone(session, command: command, error: VoiceError.publicationFailed)
                return
            }
            do {
                _ = try await session.room.localParticipant.setMicrophone(enabled: enabled)
                guard self?.isCurrent(session, command: command) == true, !Task.isCancelled else {
                    if session.retired {
                        await session.room.disconnect()
                    } else if enabled {
                        // Superseded enable is muted before the serialized newer command can run.
                        _ = try? await session.room.localParticipant.setMicrophone(enabled: false)
                    }
                    return
                }
                let publication = session.microphonePublication
                let actual = publication?.track != nil && publication?.isMuted == false
                if actual == enabled {
                    self?.reportMicrophone(session, command: command, error: nil)
                } else {
                    self?.publicationFailed(session)
                }
            } catch {
                if self?.isCurrent(session) == true {
                    // SDK capture may have started before a failing/cancelled publish produced a
                    // publication. Room track enumeration alone cannot prove that capture stopped.
                    self?.publicationFailed(session)
                } else if session.retired {
                    await session.room.disconnect()
                }
            }
        }
    }

    func disconnect() {
        precondition(Thread.isMainThread)
        guard let session = current else { return }
        // Retire synchronously, before scheduling SDK teardown or allowing another session to join.
        current = nil
        session.retired = true
        session.observer = nil
        let join = session.joinTask
        let microphone = session.microphoneTask
        join?.cancel()
        microphone?.cancel()
        let previousCleanup = cleanupTail
        cleanupTail = Task { @MainActor [weak self, session] in
            await previousCleanup?.value
            await session.room.disconnect()
            await join?.value
            await microphone?.value
            // Final teardown after in-flight work settles fences capture created by a late completion.
            await session.room.disconnect()
            // Stop SDK-global recording after every owned operation settles and before another
            // Room may connect. This synchronous SDK call blocks its RTC worker, so run it off UI.
            let recordingStopped = await Task.detached(priority: .userInitiated) {
                do {
                    try AudioManager.shared.stopLocalRecording()
                    return true
                } catch {
                    return false
                }
            }.value
            if !recordingStopped { self?.cleanupFailed = true }
            session.joinTask = nil
            session.microphoneTask = nil
        }
    }

    func close() {
        precondition(Thread.isMainThread)
        guard !closed else { return }
        closed = true
        disconnect()
        for token in notificationTokens { NotificationCenter.default.removeObserver(token) }
        notificationTokens.removeAll()
    }

    deinit {
        for token in notificationTokens { NotificationCenter.default.removeObserver(token) }
    }

    private func isCurrent(_ session: VoiceSession, command: Int64? = nil) -> Bool {
        precondition(Thread.isMainThread)
        return !closed && current === session && !session.retired &&
            (command == nil || session.microphoneCommand == command)
    }

    fileprivate func reportConnection(_ session: VoiceSession, _ state: VoiceConnection, error: VoiceError? = nil) {
        guard isCurrent(session) else { return }
        session.observer?.onConnection(attempt: session.attempt, connection: state, error: error)
    }

    private func reportMicrophone(_ session: VoiceSession, command: Int64, error: VoiceError?) {
        guard isCurrent(session, command: command) else { return }
        let publication = session.microphonePublication
        let actual = publication?.track != nil && publication?.isMuted == false
        session.microphoneEnabled = actual
        session.microphoneTask = nil
        session.observer?.onMicrophone(attempt: session.attempt, command: command, enabled: actual, error: error)
    }

    private func publicationFailed(_ session: VoiceSession) {
        guard isCurrent(session) else { return }
        reportConnection(session, .failed, error: VoiceError.publicationFailed)
        if isCurrent(session) { disconnect() }
    }

    private func audioInterrupted() {
        precondition(Thread.isMainThread)
        guard let session = current, !closed else { return }
        session.observer?.onAudioInterruption(attempt: session.attempt)
        // Controller retirement is synchronous; fallback cleanup also protects non-controller users.
        if current === session { disconnect() }
    }

    fileprivate func externalMicrophoneChange(_ session: VoiceSession) {
        guard isCurrent(session), session.microphoneTask == nil else { return }
        let publication = session.microphonePublication
        let actual = publication?.track != nil && publication?.isMuted == false
        guard actual != session.microphoneEnabled else { return }
        // Unsolicited server mute/unmute cannot silently replace the user's desired microphone state.
        audioInterrupted()
    }

    fileprivate func authorityChanged(_ session: VoiceSession) {
        guard isCurrent(session), session.room.connectionState == .connected else { return }
        if !session.room.localParticipant.permissions.canSubscribe ||
            (session.canPublish && !session.canPublishMicrophone) {
            audioInterrupted()
        }
    }

    fileprivate func reconnectStarted(_ session: VoiceSession, mode: ReconnectMode) {
        guard isCurrent(session) else { return }
        reportConnection(session, .reconnecting)
        // In 2.17.0 `.full` asynchronously republishes local tracks after connected is reported.
        // This bounded host requires a fresh explicit join, with mic off, instead of accepting that
        // implicit microphone re-publication. Quick reconnect and receive-only full reconnect remain.
        if mode == .full && (session.microphoneEnabled || session.microphoneTask != nil) {
            reportConnection(session, .failed, error: VoiceError.connectionFailed)
            if isCurrent(session) { disconnect() }
        }
    }

    private static func requestMicrophonePermission() async -> Bool {
        if #available(iOS 17.0, *) {
            switch AVAudioApplication.shared.recordPermission {
            case .granted: return true
            case .denied: return false
            case .undetermined:
                return await withCheckedContinuation { continuation in
                    AVAudioApplication.requestRecordPermission { continuation.resume(returning: $0) }
                }
            @unknown default: return false
            }
        }
        let session = AVAudioSession.sharedInstance()
        switch session.recordPermission {
        case .granted: return true
        case .denied: return false
        case .undetermined:
            return await withCheckedContinuation { continuation in
                session.requestRecordPermission { continuation.resume(returning: $0) }
            }
        @unknown default: return false
        }
    }
}

/// A delegate belongs to exactly one Room, so queued events carry an unambiguous attempt identity.
/// RoomDelegate is Sendable, but this session's mutable state belongs to the main thread. SDK delegate
/// entrypoints read only SDK-owned thread-safe values and marshal session/owner access to main; adapter
/// methods and owned async tasks also run on main. The unchecked conformance is limited to this bridge.
fileprivate final class VoiceSession: NSObject, RoomDelegate, @unchecked Sendable {
    weak var owner: NativeVoiceClient?
    let attempt: Int64
    let canPublish: Bool
    let expiresAt: Int64
    var observer: VoiceClientObserver?
    var retired = false
    var microphoneCommand: Int64 = 0
    var microphoneEnabled = false
    var joinTask: Task<Void, Never>?
    var microphoneTask: Task<Void, Never>?
    lazy var room = Room(delegate: self)
    var microphonePublication: LocalTrackPublication? {
        room.localParticipant.localAudioTracks.first { $0.source == .microphone }
    }
    var canPublishMicrophone: Bool {
        let permissions = room.localParticipant.permissions
        return canPublish && permissions.canPublish &&
            (permissions.canPublishSources.isEmpty || permissions.canPublishSources.contains(Track.Source.microphone.rawValue))
    }

    init(owner: NativeVoiceClient, attempt: Int64, grant: VoiceJoinGrant, observer: VoiceClientObserver) {
        self.owner = owner
        self.attempt = attempt
        canPublish = grant.canPublish
        expiresAt = grant.expiresAtEpochSeconds
        self.observer = observer
        super.init()
    }

    private func connection(_ state: VoiceConnection, error: VoiceError? = nil) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.owner?.reportConnection(self, state, error: error)
        }
    }

    func roomDidConnect(_ room: Room) { connection(.connected) }
    func room(_ room: Room, didFailToConnectWithError error: LiveKitError?) { connection(.failed, error: VoiceError.connectionFailed) }
    func room(_ room: Room, didDisconnectWithError error: LiveKitError?) { connection(.failed, error: VoiceError.connectionFailed) }
    func room(_ room: Room, didUpdateConnectionState state: ConnectionState, from old: ConnectionState) {
        switch state {
        case .connecting: connection(.connecting)
        case .connected: connection(.connected)
        case .reconnecting: connection(.reconnecting)
        case .disconnected: connection(.failed, error: VoiceError.connectionFailed)
        case .disconnecting: break
        @unknown default: connection(.failed, error: VoiceError.connectionFailed)
        }
    }
    // These callbacks include quick reconnects that legacy roomIsReconnecting/roomDidReconnect omit.
    func room(_ room: Room, didStartReconnectWithMode reconnectMode: ReconnectMode) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.owner?.reconnectStarted(self, mode: reconnectMode)
        }
    }
    func room(_ room: Room, didUpdateReconnectMode reconnectMode: ReconnectMode) {
        // A failed quick reconnect can be upgraded to full without another start notification.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.owner?.reconnectStarted(self, mode: reconnectMode)
        }
    }
    func room(_ room: Room, didCompleteReconnectWithMode reconnectMode: ReconnectMode) { connection(.connected) }
    func room(_ room: Room, participant: Participant, trackPublication: TrackPublication, didUpdateIsMuted muted: Bool) {
        guard participant is LocalParticipant, trackPublication.source == .microphone else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.owner?.externalMicrophoneChange(self)
        }
    }
    func room(_ room: Room, participant: LocalParticipant, didUnpublishTrack publication: LocalTrackPublication) {
        guard publication.source == .microphone else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.owner?.externalMicrophoneChange(self)
        }
    }
    func room(_ room: Room, participant: Participant, didUpdatePermissions permissions: ParticipantPermissions) {
        guard participant is LocalParticipant else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.owner?.authorityChanged(self)
        }
    }
}
