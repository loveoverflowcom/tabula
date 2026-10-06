package com.loveoverflow.tabula.mobile.host

/**
 * What the packaged game document is allowed to read, shared by the Android and iOS hosts.
 *
 * Both platforms serve the bundle from a virtual HTTPS-like origin instead of `file://`: the game's
 * verified loader needs a secure, same-origin document (WebCrypto SHA-256, `mode: same-origin`
 * fetches, CacheStorage), and `file://` has neither a usable origin nor a safe read scope. Every
 * request is mapped by [resolve] to a file inside the packaged bundle or refused. No network is used.
 */
object BundlePaths {
    /** Folder of the staged bundle inside the app (Android assets / iOS resources). */
    const val ROOT = "tabula-game"
    const val MANIFEST = "tabula-games.json"

    private val segment = Regex("^[A-Za-z0-9._@-]{1,128}$")
    private val immutableResource = Regex("^play/[a-z0-9-]+/resources/[0-9a-f]{64}\\.[a-z0-9]{1,12}$")

    /** Extensions the game document actually loads. Anything else is refused. */
    private val contentTypes = mapOf(
        "html" to "text/html; charset=utf-8",
        "js" to "text/javascript; charset=utf-8",
        "css" to "text/css; charset=utf-8",
        "wasm" to "application/wasm",
        "png" to "image/png",
        "ttf" to "font/ttf",
    )

    /**
     * Maps a request path (no query or fragment) to a file path relative to [ROOT], or `null`.
     * Only `/play/...` is served, so the games list and anything else in the bundle stay native-only.
     * Percent-encoding, backslashes, dot segments and empty segments are refused rather than decoded.
     */
    fun resolve(requestPath: String): String? {
        if (!requestPath.startsWith("/play/")) return null
        val relative = requestPath.removePrefix("/")
        val withIndex = if (relative.endsWith("/")) relative + "index.html" else relative
        val parts = withIndex.split("/")
        if (parts.size < 3 || parts.size > 6) return null
        if (parts.any { it == "." || it == ".." || !segment.matches(it) }) return null
        val file = parts.joinToString("/")
        return if (contentType(file) != null) file else null
    }

    fun contentType(file: String): String? = contentTypes[file.substringAfterLast('.', "")]

    /** Content-addressed resources never change; the entry document and fixed names must stay current. */
    fun cacheControl(file: String): String =
        if (immutableResource.matches(file)) "public, max-age=31536000, immutable" else "no-store"

    /**
     * Response headers added to every served file. The CSP is a floor under the verified loader: no
     * remote origin, no inline script, no eval beyond WebAssembly compilation, no framing.
     */
    fun securityHeaders(file: String): Map<String, String> = buildMap {
        put("Cache-Control", cacheControl(file))
        put("X-Content-Type-Options", "nosniff")
        put("Cross-Origin-Resource-Policy", "same-origin")
        put("Referrer-Policy", "no-referrer")
        if (file.endsWith(".html")) {
            put("Content-Security-Policy", DOCUMENT_CSP)
        }
    }

    const val DOCUMENT_CSP: String =
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; " +
            "font-src 'self'; img-src 'self' data: blob:; connect-src 'self'; " +
            "base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
}
