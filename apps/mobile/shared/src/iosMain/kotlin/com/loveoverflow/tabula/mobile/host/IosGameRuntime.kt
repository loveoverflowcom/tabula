@file:OptIn(ExperimentalForeignApi::class, BetaInteropApi::class)

package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.BridgeCodec
import com.loveoverflow.tabula.mobile.session.GameSession
import com.loveoverflow.tabula.mobile.session.SessionEffect
import kotlin.time.TimeSource
import kotlinx.cinterop.BetaInteropApi
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.ObjCSignatureOverride
import platform.CoreGraphics.CGRectMake
import platform.Foundation.NSBundle
import platform.Foundation.NSData
import platform.Foundation.NSError
import platform.Foundation.HTTPMethod
import platform.Foundation.NSHTTPURLResponse
import platform.Foundation.NSLog
import platform.Foundation.NSURL
import platform.Foundation.NSURLRequest
import platform.Foundation.dataWithContentsOfFile
import platform.UIKit.UIApplication
import platform.WebKit.WKContentWorld
import platform.WebKit.WKFrameInfo
import platform.WebKit.WKMediaCaptureType
import platform.WebKit.WKNavigation
import platform.WebKit.WKNavigationAction
import platform.WebKit.WKNavigationActionPolicy
import platform.WebKit.WKNavigationDelegateProtocol
import platform.WebKit.WKPermissionDecision
import platform.WebKit.WKScriptMessage
import platform.WebKit.WKScriptMessageHandlerProtocol
import platform.WebKit.WKSecurityOrigin
import platform.WebKit.WKUIDelegateProtocol
import platform.WebKit.WKURLSchemeHandlerProtocol
import platform.WebKit.WKURLSchemeTaskProtocol
import platform.WebKit.WKUserContentController
import platform.WebKit.WKUserScript
import platform.WebKit.WKUserScriptInjectionTime
import platform.WebKit.WKWebView
import platform.WebKit.WKWebViewConfiguration
import platform.WebKit.WKWebsiteDataStore
import platform.darwin.DISPATCH_TIME_NOW
import platform.darwin.NSObject
import platform.darwin.dispatch_after
import platform.darwin.dispatch_get_main_queue
import platform.darwin.dispatch_time

/**
 * One WKWebView presenting one packaged game document (ADR-0033), the iOS twin of the Android runtime.
 *
 * Lifecycle decisions come from the shared [GameSession]; this class owns the web view, the custom
 * scheme that serves the bundle and the script-message port. **Not executed in this change**: it
 * compiles to a Kotlin/Native klib on any OS, but running it needs macOS, Xcode and a simulator
 * or device. See `docs/verification/mobile-game-host/README.md`.
 */
internal class IosGameRuntime(
    private val game: BundledGame,
    launch: GameLaunch,
    private val notify: (GameHostEvent) -> Unit,
) : GameRuntimeControls {
    private val session = GameSession(
        granted = launch.capabilities,
        preferences = requireNotNull(launch.preferences) { "a game launch carries the shell's preferences" },
    )
    private val started = TimeSource.Monotonic.markNow()
    private var closed = false

    // The host objects are retained here: WebKit holds its delegates and handlers weakly or in cycles.
    private val port = Port(::onPortMessage)
    private val navigation = Navigation(::onNavigationFailure)
    private val mediaPermissions = DenyWebMediaCapture()
    val view: WKWebView

    init {
        val configuration = WKWebViewConfiguration()
        configuration.setURLSchemeHandler(BundleSchemeHandler(), forURLScheme = SCHEME)
        // Nothing the game stores outlives the session; local matches are not saved.
        configuration.websiteDataStore = WKWebsiteDataStore.nonPersistentDataStore()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.userContentController.addScriptMessageHandler(port, contentWorld = WKContentWorld.pageWorld, name = PORT)
        configuration.userContentController.addUserScript(
            WKUserScript(source = SHIM, injectionTime = WKUserScriptInjectionTime.WKUserScriptInjectionTimeAtDocumentStart, forMainFrameOnly = true),
        )
        view = WKWebView(frame = CGRectMake(0.0, 0.0, 0.0, 0.0), configuration = configuration)
        view.navigationDelegate = navigation
        view.UIDelegate = mediaPermissions
        NSLog("TabulaGameHost: runtime created for %s", game.id)
        NSURL.URLWithString(game.documentUrl("$SCHEME://$AUTHORITY"))?.let { view.loadRequest(NSURLRequest.requestWithURL(it)) }
            ?: run(session.onHostFailure("bad-document-url"))
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, HELLO_DEADLINE_NANOS), dispatch_get_main_queue()) {
            if (!closed) run(session.onHelloTimeout())
        }
    }

    override fun onBack(): Boolean {
        if (closed) return false
        val outcome = session.onBack()
        run(outcome.effects)
        return outcome.consumed
    }

    override fun suspend() { if (!closed) run(session.onHostSuspend()) }

    override fun resume() { if (!closed) run(session.onHostResume()) }

    override fun dispose() {
        if (closed) return
        run(session.onDispose())
        closed = true
        view.stopLoading()
        view.navigationDelegate = null
        view.UIDelegate = null
        view.configuration.userContentController.removeScriptMessageHandlerForName(PORT, contentWorld = WKContentWorld.pageWorld)
        view.configuration.userContentController.removeAllUserScripts()
        UIApplication.sharedApplication.idleTimerDisabled = false
        NSLog("TabulaGameHost: runtime disposed")
    }

    private fun onPortMessage(frame: WKFrameInfo, body: Any?) {
        if (closed) return
        val origin = frame.securityOrigin
        val text = body as? String
        if (!frame.mainFrame || origin.protocol != SCHEME || origin.host != AUTHORITY || text == null) {
            NSLog("TabulaGameHost: dropped a bridge message from an unexpected frame")
            return
        }
        run(session.onPageText(text))
    }

    private fun onNavigationFailure(reason: String) {
        if (!closed) run(session.onHostFailure(reason))
    }

    private fun run(effects: List<SessionEffect>) {
        for (effect in effects) {
            when (effect) {
                is SessionEffect.Send -> post(BridgeCodec.encode(effect.message))
                is SessionEffect.KeepAwake -> UIApplication.sharedApplication.idleTimerDisabled = effect.on
                is SessionEffect.Notify -> {
                    val event = effect.event
                    if (event is GameHostEvent.Ready) {
                        NSLog("TabulaGameHost: ready: page %d ms, host %lld ms since launch", event.bootMs, started.elapsedNow().inWholeMilliseconds)
                    }
                    notify(event)
                }
                is SessionEffect.Dropped -> NSLog("TabulaGameHost: dropped input: %s", effect.reason)
            }
        }
    }

    /** Host messages are enums and integers only, so the text is safe inside a single-quoted literal. */
    private fun post(text: String) {
        check('\'' !in text && '\\' !in text) { "host messages never contain quote or backslash" }
        view.evaluateJavaScript("(function(){var p=window.TabulaHostNative;if(p&&typeof p.onmessage==='function')p.onmessage({data:'$text'});})()", null)
    }

    private class Port(private val onMessage: (WKFrameInfo, Any?) -> Unit) : NSObject(), WKScriptMessageHandlerProtocol {
        override fun userContentController(userContentController: WKUserContentController, didReceiveScriptMessage: WKScriptMessage) {
            onMessage(didReceiveScriptMessage.frameInfo, didReceiveScriptMessage.body)
        }
    }

    /** Native mic permission never grants media capture to a game document or subframe (ADR-0037). */
    private class DenyWebMediaCapture : NSObject(), WKUIDelegateProtocol {
        override fun webView(
            webView: WKWebView,
            requestMediaCapturePermissionForOrigin: WKSecurityOrigin,
            initiatedByFrame: WKFrameInfo,
            type: WKMediaCaptureType,
            decisionHandler: (WKPermissionDecision) -> Unit,
        ) {
            decisionHandler(WKPermissionDecision.WKPermissionDecisionDeny)
        }
    }

    /** Only the bundle document may load; any other navigation is cancelled and never opened elsewhere. */
    private class Navigation(private val onFailure: (String) -> Unit) : NSObject(), WKNavigationDelegateProtocol {
        override fun webView(
            webView: WKWebView,
            decidePolicyForNavigationAction: WKNavigationAction,
            decisionHandler: (WKNavigationActionPolicy) -> Unit,
        ) {
            val url = decidePolicyForNavigationAction.request.URL
            val allowed = url != null && url.scheme == SCHEME && url.host == AUTHORITY && url.path?.startsWith("/play/") == true
            decisionHandler(
                if (allowed) WKNavigationActionPolicy.WKNavigationActionPolicyAllow
                else WKNavigationActionPolicy.WKNavigationActionPolicyCancel,
            )
        }

        @ObjCSignatureOverride
        override fun webView(webView: WKWebView, didFailProvisionalNavigation: WKNavigation?, withError: NSError) =
            onFailure("load-error-${withError.code}")

        @ObjCSignatureOverride
        override fun webView(webView: WKWebView, didFailNavigation: WKNavigation?, withError: NSError) =
            onFailure("load-error-${withError.code}")

        override fun webViewWebContentProcessDidTerminate(webView: WKWebView) = onFailure("renderer-terminated")
    }

    /** Serves `tabula-game://app/play/...` from the app bundle through [BundlePaths]; nothing else. */
    private class BundleSchemeHandler : NSObject(), WKURLSchemeHandlerProtocol {
        @ObjCSignatureOverride
        override fun webView(webView: WKWebView, startURLSchemeTask: WKURLSchemeTaskProtocol) {
            val request = startURLSchemeTask.request
            val url = request.URL
            val file = if (request.HTTPMethod == "GET" && url?.host == AUTHORITY) BundlePaths.resolve(url.path.orEmpty()) else null
            val bytes = file?.let { NSData.dataWithContentsOfFile("${NSBundle.mainBundle.resourcePath}/${BundlePaths.ROOT}/$it") }
            if (url == null || file == null || bytes == null) {
                respond(startURLSchemeTask, url ?: NSURL(string = "$SCHEME://$AUTHORITY/"), 404, mapOf("Cache-Control" to "no-store"), null)
                return
            }
            val headers = BundlePaths.securityHeaders(file) + ("Content-Type" to BundlePaths.contentType(file).orEmpty()) +
                ("Content-Length" to bytes.length.toString())
            respond(startURLSchemeTask, url, 200, headers, bytes)
        }

        @ObjCSignatureOverride
        override fun webView(webView: WKWebView, stopURLSchemeTask: WKURLSchemeTaskProtocol) = Unit

        @Suppress("UNCHECKED_CAST") // NSDictionary headerFields: Kotlin's Map<String, String> is the same bridge type.
        private fun respond(task: WKURLSchemeTaskProtocol, url: NSURL, status: Long, headers: Map<String, String>, body: NSData?) {
            task.didReceiveResponse(NSHTTPURLResponse(uRL = url, statusCode = status, HTTPVersion = "HTTP/1.1", headerFields = headers as Map<Any?, *>))
            body?.let { task.didReceiveData(it) }
            task.didFinish()
        }
    }

    companion object {
        /** A private scheme: WebKit cannot register handlers for http(s), and no network is involved. */
        const val SCHEME = "tabula-game"
        const val AUTHORITY = "app"
        const val PORT = "tabulaHost"
        private const val HELLO_DEADLINE_NANOS = 10_000_000_000L

        /** Defines the same port shape the Android host injects, for the main frame at document start. */
        private val SHIM = """
            (function(){
              var h=window.webkit&&window.webkit.messageHandlers&&window.webkit.messageHandlers.$PORT;
              if(!h||window.TabulaHostNative)return;
              var port={onmessage:null,postMessage:function(t){if(typeof t==='string')h.postMessage(t);}};
              Object.defineProperty(window,'TabulaHostNative',{value:port,writable:false,configurable:false,enumerable:false});
            })();
        """.trimIndent()
    }
}
