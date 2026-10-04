package com.loveoverflow.tabula.mobile.session

import com.loveoverflow.tabula.mobile.bridge.BridgeCodec
import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.HostMessage
import com.loveoverflow.tabula.mobile.bridge.PageMessage
import com.loveoverflow.tabula.mobile.bridge.ReplyCode
import com.loveoverflow.tabula.mobile.host.GameHostEvent

/** What the platform host must do as a result of one input. The session itself performs no I/O. */
sealed interface SessionEffect {
    /** Post [message] to the current document, if one is connected. */
    data class Send(val message: HostMessage) : SessionEffect

    /** Turn the platform's keep-screen-on on or off. */
    data class KeepAwake(val on: Boolean) : SessionEffect

    /** Tell the shell. */
    data class Notify(val event: GameHostEvent) : SessionEffect

    /** An input was refused. Diagnostic only; the platform logs it, never acts on it. */
    data class Dropped(val reason: String) : SessionEffect
}

/** Whether the host consumed a Back request or the shell should leave the screen itself. */
data class BackOutcome(val consumed: Boolean, val effects: List<SessionEffect>)

/**
 * The lifecycle of one game surface, as a pure state machine shared by Android and iOS (ADR-0033).
 *
 * One session belongs to one presented game surface. Each document load (first load, or the page's
 * own Retry reload) announces itself with `hello` and gets a new **generation**; everything the
 * page sends afterwards must carry that generation, so a message from an earlier document or from
 * a replayed request is dropped instead of acted on. A session never decides anything about the
 * match: it tracks the surface, the host's own suspend state, and which services were granted.
 *
 * Platform code feeds it inputs and executes the returned [SessionEffect]s in order.
 */
class GameSession(
    private val granted: Set<HostCapability>,
    private val preferences: GamePreferences,
) {
    enum class Phase { AwaitingPage, Starting, Ready, Failed, Closed }

    var phase: Phase = Phase.AwaitingPage
        private set
    var generation: Int = 0
        private set
    var keepAwake: Boolean = false
        private set

    private var hostSuspended = false
    private var lastServiceId = 0

    /** Raw text from the page's origin-restricted port. */
    fun onPageText(text: String): List<SessionEffect> {
        val message = BridgeCodec.decodePage(text) ?: return dropped("malformed")
        if (phase == Phase.Closed) return dropped("closed")
        return when (message) {
            PageMessage.Hello -> hello()
            is PageMessage.Ready -> current(message.generation) ?: ready(message)
            is PageMessage.Failed -> current(message.generation) ?: failed(message)
            is PageMessage.Exit -> current(message.generation) ?: exit()
            is PageMessage.Service -> current(message.generation) ?: service(message)
        }
    }

    fun onHostSuspend(): List<SessionEffect> {
        if (hostSuspended) return emptyList()
        hostSuspended = true
        return if (connected()) send(HostMessage.Suspend(generation)) else emptyList()
    }

    fun onHostResume(): List<SessionEffect> {
        if (!hostSuspended) return emptyList()
        hostSuspended = false
        return if (connected()) send(HostMessage.Resume(generation)) else emptyList()
    }

    /** Back is routed to the page's own leave confirmation only while a match is live. */
    fun onBack(): BackOutcome =
        if (phase == Phase.Ready) BackOutcome(true, send(HostMessage.BackRequested(generation)))
        else BackOutcome(false, emptyList())

    /** The platform could not present or keep the surface (load error, renderer gone, no bridge). */
    fun onHostFailure(reason: String): List<SessionEffect> {
        if (phase == Phase.Closed || phase == Phase.Failed) return dropped("already-$phase")
        phase = Phase.Failed
        return releaseKeepAwake() + SessionEffect.Notify(GameHostEvent.Failed(reason, shownByGame = false))
    }

    /** The page never said hello within the platform's deadline. */
    fun onHelloTimeout(): List<SessionEffect> =
        if (phase == Phase.AwaitingPage) onHostFailure("no-handshake") else emptyList()

    /** The surface is going away. Idempotent; a closed session has no authority left. */
    fun onDispose(): List<SessionEffect> {
        if (phase == Phase.Closed) return emptyList()
        val wasConnected = connected()
        val dispose = if (wasConnected) send(HostMessage.Dispose(generation)) else emptyList()
        phase = Phase.Closed
        return dispose + releaseKeepAwake()
    }

    private fun connected() = generation > 0 && (phase == Phase.Starting || phase == Phase.Ready)

    private fun hello(): List<SessionEffect> {
        // A reload is a new document: it restarts from Starting and forgets the old one's requests.
        generation += 1
        lastServiceId = 0
        phase = Phase.Starting
        val effects = releaseKeepAwake() + send(HostMessage.Init(generation, granted, preferences))
        return if (hostSuspended) effects + send(HostMessage.Suspend(generation)) else effects
    }

    /** `null` when [claimed] is the live generation; otherwise the drop effect. */
    private fun current(claimed: Int): List<SessionEffect>? = when {
        generation == 0 || claimed != generation -> dropped("stale-generation")
        phase == Phase.Failed -> dropped("failed")
        else -> null
    }

    private fun ready(message: PageMessage.Ready): List<SessionEffect> {
        if (phase != Phase.Starting) return dropped("duplicate-ready")
        phase = Phase.Ready
        return listOf(SessionEffect.Notify(GameHostEvent.Ready(message.bootMs)))
    }

    private fun failed(message: PageMessage.Failed): List<SessionEffect> {
        phase = Phase.Failed
        // The page keeps its own recovery overlay (Return / Try again); the shell only records it.
        return releaseKeepAwake() + SessionEffect.Notify(
            GameHostEvent.Failed("${message.code.wire}: ${message.detail}", shownByGame = true),
        )
    }

    private fun exit(): List<SessionEffect> {
        phase = Phase.Closed
        return releaseKeepAwake() + SessionEffect.Notify(GameHostEvent.Exited)
    }

    private fun service(request: PageMessage.Service): List<SessionEffect> {
        // Request ids only increase inside a document, so a replayed or reordered request is stale.
        if (request.id <= lastServiceId) return dropped("replayed-request")
        lastServiceId = request.id
        if (request.capability !in granted) return reply(request, ReplyCode.Denied)
        if (phase != Phase.Ready) return reply(request, ReplyCode.Unsupported)
        return when (request.capability) {
            HostCapability.KeepAwake -> {
                keepAwake = request.enabled
                listOf(SessionEffect.KeepAwake(request.enabled)) + reply(request, ReplyCode.Ok)
            }
        }
    }

    private fun reply(request: PageMessage.Service, code: ReplyCode) =
        send(HostMessage.Reply(generation, request.id, code == ReplyCode.Ok, code))

    private fun releaseKeepAwake(): List<SessionEffect> {
        if (!keepAwake) return emptyList()
        keepAwake = false
        return listOf(SessionEffect.KeepAwake(false))
    }

    private fun send(message: HostMessage) = listOf<SessionEffect>(SessionEffect.Send(message))

    private fun dropped(reason: String) = listOf<SessionEffect>(SessionEffect.Dropped(reason))
}
