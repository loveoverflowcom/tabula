import SwiftUI
import Foundation
import TabulaShared

@main
struct TabulaApp: App {
    var body: some Scene { WindowGroup { TabulaRoot().ignoresSafeArea() } }
}

/// Thin container: the Compose Multiplatform view controller owns UI and navigation.
struct TabulaRoot: UIViewControllerRepresentable {
    final class Coordinator {
        let voice = NativeVoiceClient()

        deinit {
            let client = voice
            if Thread.isMainThread { client.close() }
            else { DispatchQueue.main.async { client.close() } }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeUIViewController(context: Context) -> UIViewController {
        TabulaIosKt.TabulaViewController(voiceClient: context.coordinator.voice, developmentGrantText: Self.developmentGrantText())
    }
    func updateUIViewController(_ controller: UIViewController, context: Context) {}

    static func dismantleUIViewController(_ controller: UIViewController, coordinator: Coordinator) {
        coordinator.voice.close()
    }

    private static func developmentGrantText() -> String? {
        #if DEBUG
        // This optional fixture is copied by the Debug-only build phase, never a credential fallback.
        guard let url = Bundle.main.url(forResource: "voice-dev-grant", withExtension: "json"),
            let size = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize,
            size <= 20_000, let data = try? Data(contentsOf: url), data.count <= 20_000 else { return nil }
        return String(data: data, encoding: .utf8)
        #else
        return nil
        #endif
    }
}
