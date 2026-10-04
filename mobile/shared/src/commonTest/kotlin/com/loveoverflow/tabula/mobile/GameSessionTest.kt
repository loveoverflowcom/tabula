package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.HostMessage
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ReplyCode
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.session.GameSession
import com.loveoverflow.tabula.mobile.session.GameSession.Phase
import com.loveoverflow.tabula.mobile.session.SessionEffect
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class GameSessionTest {
    private val prefs = GamePreferences(ThemePreference.Dark, MotionPreference.Reduced, LocalePreference.English)

    private fun session(vararg granted: HostCapability) = GameSession(granted.toSet(), prefs)

    private fun hello() = """{"v":1,"type":"hello"}"""
    private fun ready(gen: Int) = """{"v":1,"type":"ready","gen":$gen,"bootMs":900}"""
    private fun exit(gen: Int) = """{"v":1,"type":"exit","gen":$gen}"""
    private fun failed(gen: Int) = """{"v":1,"type":"failed","gen":$gen,"code":"runtime","detail":"boom"}"""
    private fun keepAwake(gen: Int, id: Int, on: Boolean = true) =
        """{"v":1,"type":"service","gen":$gen,"id":$id,"name":"keep-awake","enabled":$on}"""

    private fun List<SessionEffect>.sent() = filterIsInstance<SessionEffect.Send>().map { it.message }
    private fun List<SessionEffect>.events() = filterIsInstance<SessionEffect.Notify>().map { it.event }
    private fun List<SessionEffect>.dropped() = filterIsInstance<SessionEffect.Dropped>().map { it.reason }

    private fun GameSession.started(): GameSession = also { onPageText(hello()); onPageText(ready(generation)) }

    @Test
    fun helloStartsAGenerationAndAnswersWithGrantedCapabilitiesAndPreferences() {
        val s = session(HostCapability.KeepAwake)
        assertEquals(Phase.AwaitingPage, s.phase)
        val effects = s.onPageText(hello())
        assertEquals(listOf<HostMessage>(HostMessage.Init(1, setOf(HostCapability.KeepAwake), prefs)), effects.sent())
        assertEquals(Phase.Starting, s.phase)
    }

    @Test
    fun readyIsReportedOnceAndOnlyForTheLiveGeneration() {
        val s = session()
        s.onPageText(hello())
        assertEquals(listOf<GameHostEvent>(GameHostEvent.Ready(900)), s.onPageText(ready(1)).events())
        assertEquals(Phase.Ready, s.phase)
        assertEquals(listOf("duplicate-ready"), s.onPageText(ready(1)).dropped())
        assertEquals(listOf("stale-generation"), s.onPageText(ready(2)).dropped())
    }

    @Test
    fun messagesBeforeHelloOrFromAnEarlierDocumentAreDropped() {
        val s = session()
        assertEquals(listOf("stale-generation"), s.onPageText(ready(1)).dropped())
        s.onPageText(hello())
        s.onPageText(hello()) // the page reloaded: generation 2 supersedes 1
        assertEquals(2, s.generation)
        assertEquals(listOf("stale-generation"), s.onPageText(exit(1)).dropped())
        assertEquals(Phase.Starting, s.phase)
        assertEquals(listOf("stale-generation"), s.onPageText(failed(1)).dropped())
    }

    @Test
    fun malformedOrOversizedTextNeverChangesState() {
        val s = session().started()
        for (text in listOf("", "not json", """{"v":1,"type":"exit","gen":1,"x":1}""", """{"v":2,"type":"exit","gen":1}""")) {
            assertEquals(listOf("malformed"), s.onPageText(text).dropped())
        }
        assertEquals(Phase.Ready, s.phase)
    }

    @Test
    fun exitNotifiesTheShellOnceAndClosesTheSession() {
        val s = session().started()
        assertEquals(listOf<GameHostEvent>(GameHostEvent.Exited), s.onPageText(exit(1)).events())
        assertEquals(Phase.Closed, s.phase)
        // A late event after the player left has no effect.
        assertEquals(listOf("closed"), s.onPageText(exit(1)).dropped())
        assertEquals(listOf("closed"), s.onPageText(ready(1)).dropped())
        assertEquals(listOf("closed"), s.onPageText(hello()).dropped())
    }

    @Test
    fun pageFailureIsRecordedAsShownByTheGameAndRetryStartsAFreshGeneration() {
        val s = session().started()
        val effects = s.onPageText(failed(1))
        assertEquals(listOf<GameHostEvent>(GameHostEvent.Failed("runtime: boom", shownByGame = true)), effects.events())
        assertEquals(Phase.Failed, s.phase)
        // Everything else from the failed document is ignored...
        assertEquals(listOf("failed"), s.onPageText(ready(1)).dropped())
        // ...until the page's own Try again reloads and says hello.
        assertEquals(listOf<HostMessage>(HostMessage.Init(2, emptySet(), prefs)), s.onPageText(hello()).sent())
        assertEquals(Phase.Starting, s.phase)
    }

    @Test
    fun hostFailureIsNotShownByTheGameAndIsReportedOnce() {
        val s = session().started()
        val first = s.onHostFailure("renderer-killed")
        assertEquals(listOf<GameHostEvent>(GameHostEvent.Failed("renderer-killed", shownByGame = false)), first.events())
        assertEquals(1, s.onHostFailure("renderer-killed").dropped().size)
    }

    @Test
    fun silentPageFailsClosedAfterTheHandshakeDeadlineButOnlyWhileWaiting() {
        val waiting = session()
        assertEquals(listOf<GameHostEvent>(GameHostEvent.Failed("no-handshake", shownByGame = false)), waiting.onHelloTimeout().events())
        val answered = session()
        answered.onPageText(hello())
        assertTrue(answered.onHelloTimeout().isEmpty())
        assertEquals(Phase.Starting, answered.phase)
    }

    @Test
    fun suspendAndResumeAreIdempotentAndOnlyReachAConnectedPage() {
        val s = session()
        assertTrue(s.onHostSuspend().isEmpty(), "no document yet")
        assertTrue(s.onHostResume().isEmpty())
        s.started()
        assertEquals(listOf<HostMessage>(HostMessage.Suspend(1)), s.onHostSuspend().sent())
        assertTrue(s.onHostSuspend().isEmpty())
        assertEquals(listOf<HostMessage>(HostMessage.Resume(1)), s.onHostResume().sent())
        assertTrue(s.onHostResume().isEmpty())
    }

    @Test
    fun aDocumentThatStartsWhileTheAppIsBackgroundedIsToldToSuspendRightAfterInit() {
        val s = session()
        s.onHostSuspend()
        assertEquals(
            listOf(HostMessage.Init(1, emptySet(), prefs), HostMessage.Suspend(1)),
            s.onPageText(hello()).sent(),
        )
    }

    @Test
    fun backIsRoutedToTheGameOnlyWhileAMatchIsLive() {
        val s = session()
        assertFalse(s.onBack().consumed, "nothing loaded: the shell leaves")
        s.onPageText(hello())
        assertFalse(s.onBack().consumed, "still loading: the shell leaves")
        s.onPageText(ready(1))
        val live = s.onBack()
        assertTrue(live.consumed)
        assertEquals(listOf<HostMessage>(HostMessage.BackRequested(1)), live.effects.filterIsInstance<SessionEffect.Send>().map { it.message })
        s.onPageText(failed(1))
        assertFalse(s.onBack().consumed, "a failed game cannot confirm a leave")
    }

    @Test
    fun keepAwakeIsGrantedOnlyWhenTheLaunchAllowsItAndOnlyInAReadyDocument() {
        val granted = session(HostCapability.KeepAwake)
        granted.onPageText(hello())
        val early = granted.onPageText(keepAwake(1, 1))
        assertEquals(listOf<HostMessage>(HostMessage.Reply(1, 1, false, ReplyCode.Unsupported)), early.sent())
        assertFalse(granted.keepAwake)
        granted.onPageText(ready(1))
        val ok = granted.onPageText(keepAwake(1, 2))
        assertEquals(listOf<SessionEffect>(SessionEffect.KeepAwake(true)), ok.filterIsInstance<SessionEffect.KeepAwake>())
        assertEquals(listOf<HostMessage>(HostMessage.Reply(1, 2, true, ReplyCode.Ok)), ok.sent())
        assertTrue(granted.keepAwake)

        val denied = session().started()
        val refused = denied.onPageText(keepAwake(1, 1))
        assertEquals(listOf<HostMessage>(HostMessage.Reply(1, 1, false, ReplyCode.Denied)), refused.sent())
        assertTrue(refused.none { it is SessionEffect.KeepAwake })
    }

    @Test
    fun replayedOrReorderedServiceRequestsAreDropped() {
        val s = session(HostCapability.KeepAwake).started()
        s.onPageText(keepAwake(1, 5))
        assertEquals(listOf("replayed-request"), s.onPageText(keepAwake(1, 5, on = false)).dropped())
        assertEquals(listOf("replayed-request"), s.onPageText(keepAwake(1, 3, on = false)).dropped())
        assertTrue(s.keepAwake, "a replay must not switch the screen setting back")
    }

    @Test
    fun keepAwakeIsReleasedByExitFailureReloadAndDispose() {
        fun awake() = session(HostCapability.KeepAwake).started().also { it.onPageText(keepAwake(1, 1)) }
        assertTrue(awake().onPageText(exit(1)).contains(SessionEffect.KeepAwake(false)))
        assertTrue(awake().onPageText(failed(1)).contains(SessionEffect.KeepAwake(false)))
        assertTrue(awake().onHostFailure("renderer-killed").contains(SessionEffect.KeepAwake(false)))
        assertTrue(awake().onDispose().contains(SessionEffect.KeepAwake(false)))
        val reloaded = awake()
        assertTrue(reloaded.onPageText(hello()).contains(SessionEffect.KeepAwake(false)), "a new document starts without the old one's setting")
        assertFalse(reloaded.keepAwake)
    }

    @Test
    fun disposeIsIdempotentTellsTheLivePageAndRemovesAllAuthority() {
        val s = session(HostCapability.KeepAwake).started()
        assertEquals(listOf<HostMessage>(HostMessage.Dispose(1)), s.onDispose().sent())
        assertEquals(Phase.Closed, s.phase)
        assertTrue(s.onDispose().isEmpty())
        assertTrue(s.onHostSuspend().isEmpty())
        assertTrue(s.onHostFailure("late").none { it is SessionEffect.Notify })
        assertFalse(s.onBack().consumed)
        assertEquals(listOf("closed"), s.onPageText(keepAwake(1, 9)).dropped())
    }

    @Test
    fun disposeBeforeAnyPageSpokeSendsNothing() {
        assertTrue(session().onDispose().isEmpty())
    }
}
