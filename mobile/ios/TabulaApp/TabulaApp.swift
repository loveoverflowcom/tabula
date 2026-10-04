import SwiftUI
import TabulaShared

@main
struct TabulaApp: App {
    var body: some Scene { WindowGroup { TabulaRoot().ignoresSafeArea() } }
}

/// Thin container: the Compose Multiplatform view controller owns UI and navigation.
struct TabulaRoot: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> UIViewController {
        TabulaIosKt.TabulaViewController()
    }
    func updateUIViewController(_ controller: UIViewController, context: Context) {}
}
