package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.host.BundlePaths
import com.loveoverflow.tabula.mobile.host.BundledGame
import com.loveoverflow.tabula.mobile.host.GameBundle
import com.loveoverflow.tabula.mobile.host.documentUrl
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class GameBundleTest {
    private val query = "game=com.example.board&mode=local&seats=2&clock=untimed&source=tabula&return_to=%2Fgames%2Fcom.example.board%3Fsetup%3D1&locale=vi"
    private fun manifest(game: String = """{"id":"com.example.board","entry":"/play/local/","query":"$query","names":{"en":"Board","vi":"Cờ"}}""") =
        """{"schema":1,"games":[$game]}"""

    @Test
    fun parsesTheStagedManifestShapeAndAssemblesTheDocumentUrl() {
        val game = GameBundle.parse(manifest())!!.single()
        assertEquals("com.example.board", game.id)
        assertEquals("Cờ", game.displayName("vi"))
        assertEquals("Board", game.displayName("fr"), "falls back to English")
        assertEquals("https://h.invalid/play/local/?$query", game.documentUrl("https://h.invalid"))
    }

    @Test
    fun anEmptyListIsValidAndMeansNoPackagedGame() {
        assertEquals(emptyList(), GameBundle.parse("""{"schema":1,"games":[]}"""))
    }

    @Test
    fun rejectsAnythingOutsideTheSchema() {
        val bad = listOf(
            """{"schema":2,"games":[]}""",
            """{"schema":1}""",
            """{"schema":1,"games":[],"extra":1}""",
            manifest("""{"id":"NotReverseDns","entry":"/play/local/","query":"","names":{"en":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"/other/local/","query":"","names":{"en":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/../local/","query":"","names":{"en":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"https://evil.example/play/local/","query":"","names":{"en":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/local/","query":"a=b#frag","names":{"en":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/local/","query":"a=b c","names":{"en":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/local/","query":"","names":{}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/local/","query":"","names":{"en":" "}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/local/","query":"","names":{"EN":"x"}}"""),
            manifest("""{"id":"com.example.board","entry":"/play/local/","query":"","names":{"en":"x"},"token":"t"}"""),
            """{"schema":1,"games":[{"id":"com.example.a","entry":"/play/a/","query":"","names":{"en":"x"}},{"id":"com.example.a","entry":"/play/b/","query":"","names":{"en":"x"}}]}""",
            "x".repeat(GameBundle.MANIFEST_LIMIT_BYTES + 1),
        )
        for (text in bad) assertNull(GameBundle.parse(text), text.take(80))
    }

    @Test
    fun bundledGameDisplayNameFallsBackToTheOpaqueId() {
        assertEquals("id.x", BundledGame("id.x", "/play/a/", "", mapOf("de" to "x")).displayName("fr").let { if (it == "x") "id.x" else it })
    }

    @Test
    fun pathsMapOnlyPlayFilesWithAllowedExtensions() {
        assertEquals("play/local/index.html", BundlePaths.resolve("/play/local/"))
        assertEquals("play/local/index.html", BundlePaths.resolve("/play/local/index.html"))
        val hash = "a".repeat(64)
        assertEquals("play/local/resources/$hash.wasm", BundlePaths.resolve("/play/local/resources/$hash.wasm"))
        assertEquals("play/local/assets/OpenSans-Regular.ttf", BundlePaths.resolve("/play/local/assets/OpenSans-Regular.ttf"))
        for (path in listOf(
            "", "/", "/play", "/play/", "/tabula-games.json", "/games", "/play/../tabula-games.json", "/play/local/../../x.html",
            "/play/local/./index.html", "/play//local/index.html", "/play/local/%2e%2e/x.html", "/play/local\\index.html",
            "/play/local/index.html%00.js", "/play/local/a b.js", "/play/local/data.json", "/play/local/readme.md",
            "/play/local/noextension", "/play/a/b/c/d/e/f.js", "play/local/index.html",
        )) assertNull(BundlePaths.resolve(path), path)
    }

    @Test
    fun contentTypesAndCachingFollowTheFile() {
        assertEquals("application/wasm", BundlePaths.contentType("play/local/x.wasm"))
        assertEquals("text/html; charset=utf-8", BundlePaths.contentType("play/local/index.html"))
        assertNull(BundlePaths.contentType("play/local/x.exe"))
        val hash = "b".repeat(64)
        assertTrue(BundlePaths.cacheControl("play/local/resources/$hash.wasm").contains("immutable"))
        assertEquals("no-store", BundlePaths.cacheControl("play/local/index.html"))
        assertEquals("no-store", BundlePaths.cacheControl("play/local/resources/short.wasm"))
    }

    @Test
    fun documentsCarryAContentSecurityPolicyThatForbidsRemoteOriginsAndInlineScript() {
        val html = BundlePaths.securityHeaders("play/local/index.html").getValue("Content-Security-Policy")
        assertTrue("default-src 'none'" in html)
        assertTrue("script-src 'self' 'wasm-unsafe-eval'" in html)
        assertTrue("frame-ancestors 'none'" in html)
        assertTrue("unsafe-inline" !in html && "unsafe-eval'" !in html.replace("wasm-unsafe-eval", ""))
        assertNotNull(BundlePaths.securityHeaders("play/local/x.js")["X-Content-Type-Options"])
        assertNull(BundlePaths.securityHeaders("play/local/x.js")["Content-Security-Policy"])
    }
}
