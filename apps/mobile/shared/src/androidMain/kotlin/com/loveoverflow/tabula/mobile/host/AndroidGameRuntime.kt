package com.loveoverflow.tabula.mobile.host

import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.res.AssetManager
import android.net.Uri
import android.util.Log
import android.webkit.PermissionRequest
import android.webkit.WebChromeClient
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import com.loveoverflow.tabula.mobile.bridge.BridgeCodec
import com.loveoverflow.tabula.mobile.session.GameSession
import com.loveoverflow.tabula.mobile.session.SessionEffect
import java.io.ByteArrayInputStream
import java.io.FileNotFoundException
import java.util.concurrent.atomic.AtomicInteger

/**
 * One Android WebView presenting one packaged game document (ADR-0033).
 *
 * The runtime is created exactly once when its composable enters the composition and disposed
 * exactly once when it leaves; [Counters] records both so tests can prove a recomposition does not
 * build a second one. All lifecycle decisions are made by the shared [GameSession]; this class only
 * performs the effects it returns, and owns the WebView and the transport.
 */
internal class AndroidGameRuntime(
    context: Context,
    private val game: BundledGame,
    launch: GameLaunch,
    private val assets: AssetManager,
    private val notify: (GameHostEvent) -> Unit,
) : GameRuntimeControls {
    /** Instances ever built and ever disposed in this process. Diagnostic and test evidence only. */
    object Counters {
        val created = AtomicInteger()
        val disposed = AtomicInteger()
    }

    private val id = Counters.created.incrementAndGet()
    private val session = GameSession(
        granted = launch.capabilities,
        preferences = requireNotNull(launch.preferences) { "a game launch carries the shell's preferences" },
    )
    private var reply: JavaScriptReplyProxy? = null
    private var closed = false
    private val startedAtNanos = System.nanoTime()
    private val helloDeadline = Runnable { run(session.onHelloTimeout()) }

    val view: WebView = WebView(context)

    init {
        Log.i(TAG, "runtime#$id created for ${game.id}")
        configure(context)
        // The bridge port is injected only into frames of the bundle origin; nothing else sees it.
        WebViewCompat.addWebMessageListener(view, PORT, setOf(ORIGIN)) { _, message: WebMessageCompat, source: Uri, isMainFrame, proxy ->
            onPortMessage(message, source, isMainFrame, proxy)
        }
        view.webViewClient = Client()
        // Native RECORD_AUDIO permission never grants capture to the game document (ADR-0037).
        view.webChromeClient = object : WebChromeClient() {
            override fun onPermissionRequest(request: PermissionRequest) { request.deny() }
        }
        view.loadUrl(game.documentUrl(ORIGIN))
        view.postDelayed(helloDeadline, HELLO_DEADLINE_MS)
    }

    @SuppressLint("SetJavaScriptEnabled") // The game is Rust/WASM driven through JavaScript.
    private fun configure(context: Context) {
        val debuggable = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
        WebView.setWebContentsDebuggingEnabled(debuggable)
        with(view.settings) {
            javaScriptEnabled = true
            // The bundle is reached only through request interception, never a file or network read.
            allowFileAccess = false
            allowContentAccess = false
            blockNetworkLoads = true
            setSupportMultipleWindows(false)
            javaScriptCanOpenWindowsAutomatically = false
            mediaPlaybackRequiresUserGesture = true
            setGeolocationEnabled(false)
            cacheMode = WebSettings.LOAD_DEFAULT
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
        }
    }

    /** `true` when the page will show its own leave confirmation; `false` when the shell should pop. */
    override fun onBack(): Boolean {
        if (closed) return false
        val outcome = session.onBack()
        run(outcome.effects)
        return outcome.consumed
    }

    override fun suspend() {
        if (closed) return
        run(session.onHostSuspend())
        view.onPause()
    }

    override fun resume() {
        if (closed) return
        view.onResume()
        run(session.onHostResume())
    }

    /** Idempotent. After this nothing the page sends has any effect. */
    override fun dispose() {
        if (closed) return
        run(session.onDispose())
        closed = true
        reply = null
        view.removeCallbacks(helloDeadline)
        view.keepScreenOn = false
        WebViewCompat.removeWebMessageListener(view, PORT)
        view.stopLoading()
        view.loadUrl("about:blank")
        // destroy() requires a detached view; Compose's own removal may not have happened yet.
        (view.parent as? android.view.ViewGroup)?.removeView(view)
        view.destroy()
        Counters.disposed.incrementAndGet()
        Log.i(TAG, "runtime#$id disposed")
    }

    private fun onPortMessage(message: WebMessageCompat, source: Uri, isMainFrame: Boolean, proxy: JavaScriptReplyProxy) {
        if (closed) return
        val text = message.data
        if (!isMainFrame || !isBundleOrigin(source) || message.type != WebMessageCompat.TYPE_STRING || text == null) {
            Log.w(TAG, "runtime#$id dropped a bridge message (main=$isMainFrame origin=$source)")
            return
        }
        reply = proxy
        run(session.onPageText(text))
    }

    private fun run(effects: List<SessionEffect>) {
        for (effect in effects) {
            when (effect) {
                is SessionEffect.Send -> reply?.postMessage(BridgeCodec.encode(effect.message))
                is SessionEffect.KeepAwake -> view.keepScreenOn = effect.on
                is SessionEffect.Notify -> {
                    if (effect.event is GameHostEvent.Ready) {
                        val ms = (System.nanoTime() - startedAtNanos) / 1_000_000
                        Log.i(TAG, "runtime#$id ready: page ${effect.event.bootMs} ms, host ${ms} ms since launch")
                    }
                    notify(effect.event)
                }
                is SessionEffect.Dropped -> Log.d(TAG, "runtime#$id dropped input: ${effect.reason}")
            }
        }
    }

    private inner class Client : WebViewClient() {
        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse =
            serve(request)

        // Only the bundle document may load; everything else is cancelled, never opened elsewhere.
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
            !isBundleOrigin(request.url)

        override fun onPageStarted(view: WebView, url: String?, favicon: android.graphics.Bitmap?) {
            // A new document: the previous document's reply proxy is no longer valid.
            reply = null
        }

        override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
            if (request.isForMainFrame && !closed) run(session.onHostFailure("load-error-${error.errorCode}"))
        }

        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            if (!closed) run(session.onHostFailure(if (detail.didCrash()) "renderer-crashed" else "renderer-killed"))
            return true // handled: the app must not die with its renderer
        }
    }

    private fun serve(request: WebResourceRequest): WebResourceResponse {
        val url = request.url
        if (request.method != "GET" || !isBundleOrigin(url)) return refusal(403, "Forbidden")
        val file = BundlePaths.resolve(url.path.orEmpty()) ?: return refusal(404, "Not Found")
        val type = BundlePaths.contentType(file) ?: return refusal(404, "Not Found")
        return try {
            WebResourceResponse(
                type.substringBefore(';'),
                if (type.startsWith("text/")) "utf-8" else null,
                200,
                "OK",
                BundlePaths.securityHeaders(file),
                assets.open("${BundlePaths.ROOT}/$file"),
            )
        } catch (_: FileNotFoundException) {
            refusal(404, "Not Found")
        }
    }

    private fun refusal(status: Int, reason: String) = WebResourceResponse(
        "text/plain", "utf-8", status, reason, mapOf("Cache-Control" to "no-store"), ByteArrayInputStream(reason.toByteArray()),
    )

    companion object {
        const val TAG = "TabulaGameHost"
        const val PORT = "TabulaHostNative"
        /** `.invalid` never resolves, so a request that escaped interception could not reach any server. */
        const val ORIGIN = "https://game.tabula.invalid"
        private const val HELLO_DEADLINE_MS = 10_000L

        fun isBundleOrigin(uri: Uri): Boolean =
            uri.scheme == "https" && uri.host == "game.tabula.invalid" && (uri.port == -1 || uri.port == 443)
    }
}
