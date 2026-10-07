package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ReplyCode
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.host.NativeGameRuntime
import com.loveoverflow.tabula.mobile.host.NativePointer
import com.loveoverflow.tabula.mobile.host.NativeRuntimeCommand
import com.loveoverflow.tabula.mobile.host.NativeRuntimeEvent
import com.loveoverflow.tabula.mobile.host.NativeRuntimeId
import com.loveoverflow.tabula.mobile.host.NativeRuntimeOpen
import com.loveoverflow.tabula.mobile.host.NativeRuntimeOwner
import com.loveoverflow.tabula.mobile.host.NativeRuntimePort
import com.loveoverflow.tabula.mobile.host.NativeSurface
import com.loveoverflow.tabula.mobile.host.NativeSurfaceGeometry
import com.loveoverflow.tabula.mobile.host.NativeSurfaceLease
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Executes the real admission/effect coordinator with a controlled port (ADR-0043, I-9/I-10).
 * These are lifecycle assertions, not GPU, native packaging, frame-pacing or device evidence.
 */
class NativeGameRuntimeTest {
    private companion object {
        const val FIXTURE_ID = "com.example.fixture"
        val preferences = GamePreferences(ThemePreference.Dark, MotionPreference.Reduced, LocalePreference.English)
        val geometry = assertNotNull(NativeSurfaceGeometry.create(640, 480, 2f))
        val resizedGeometry = assertNotNull(NativeSurfaceGeometry.create(480, 640, 3f))
    }

    /** No native code, render loop or GPU is hidden behind this port. */
    private class Port : NativeRuntimePort {
        val commands = mutableListOf<NativeRuntimeCommand>()
        val fences = mutableListOf<NativeSurfaceLease>()
        var onCommand: (NativeRuntimeCommand, (NativeRuntimeEvent) -> Unit) -> Unit = { _, _ -> }
        var onFence: (NativeSurfaceLease) -> Unit = {}
        private var callback: ((NativeRuntimeEvent) -> Unit)? = null
        private val callbacks = mutableMapOf<NativeRuntimeId, (NativeRuntimeEvent) -> Unit>()
        private var executionDepth = 0
        var maximumExecutionDepth = 0
            private set

        override fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit) {
            callback = emit
            callbacks[command.runtime] = emit
            commands += command
            executionDepth += 1
            maximumExecutionDepth = maxOf(maximumExecutionDepth, executionDepth)
            try {
                onCommand(command, emit)
            } finally {
                executionDepth -= 1
            }
        }

        override fun fenceSurface(lease: NativeSurfaceLease) {
            fences += lease
            onFence(lease)
        }

        fun emit(event: NativeRuntimeEvent) = assertNotNull(callback)(event)
        fun callbackFor(runtime: NativeRuntimeId): (NativeRuntimeEvent) -> Unit = assertNotNull(callbacks[runtime])
        inline fun <reified T : NativeRuntimeCommand> sent(): List<T> = commands.filterIsInstance<T>()
    }

    private class Rig(
        val launch: GameLaunch = GameLaunch(FIXTURE_ID, preferences),
        packagedIds: Set<String> = setOf(FIXTURE_ID),
        supported: Set<HostCapability> = setOf(HostCapability.KeepAwake),
    ) {
        val port = Port()
        val events = mutableListOf<GameHostEvent>()
        val awake = mutableListOf<Boolean>()
        var now = 100L
        var onEvent: (GameHostEvent) -> Unit = {}
        var onAwake: (Boolean) -> Unit = {}
        var onOwnerThread = true
        var ownerChecks = 0
        val owner = NativeRuntimeOwner(packagedIds, supported) {
            ownerChecks += 1
            check(onOwnerThread) { "wrong-owner-thread" }
        }

        fun tryOpen(value: GameLaunch = launch, foreground: Boolean = true): NativeRuntimeOpen = owner.open(
            value, port, foreground, { now },
            { events += it; onEvent(it) },
            { awake += it; onAwake(it) },
        )

        fun open(value: GameLaunch = launch, foreground: Boolean = true): NativeGameRuntime =
            assertIs<NativeRuntimeOpen.Opened>(tryOpen(value, foreground)).runtime

        fun attach(runtime: NativeGameRuntime): NativeSurfaceLease =
            assertNotNull(runtime.attach(object : NativeSurface {}, geometry))

        fun prerequisites(lease: NativeSurfaceLease) {
            port.emit(NativeRuntimeEvent.ContextReady(lease))
            port.emit(NativeRuntimeEvent.ResourcesReady(lease))
            port.emit(NativeRuntimeEvent.InputReady(lease))
        }

        fun interactive(lease: NativeSurfaceLease, frame: Long = 1) {
            prerequisites(lease)
            port.emit(NativeRuntimeEvent.FramePresented(lease, frame))
        }

        fun ready(): List<GameHostEvent.Ready> = events.filterIsInstance<GameHostEvent.Ready>()
        fun failures(): List<GameHostEvent.Failed> = events.filterIsInstance<GameHostEvent.Failed>()
    }

    @Test
    fun invalidOpaqueIdsNeverStartEvenWhenListedInInventory() {
        val invalid = listOf("", "fixture", "Com.example.fixture", "com.example/fixture", "com..fixture", " com.example.fixture", "com.example.fixture\n", "com.example." + "a".repeat(70))
        for (id in invalid) {
            val rig = Rig(packagedIds = setOf(id))
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen(GameLaunch(id, preferences)), id)
            assertTrue(rig.port.commands.isEmpty(), id)
            assertTrue(rig.events.isEmpty(), id)
        }
    }

    @Test
    fun unpackagedNullOrUnresolvedPreferencesAndUnsupportedCapabilitiesNeverStart() {
        val denied = listOf(
            Rig(packagedIds = emptySet()),
            Rig(GameLaunch(FIXTURE_ID)),
            Rig(GameLaunch(FIXTURE_ID, preferences.copy(theme = ThemePreference.System))),
            Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)), supported = emptySet()),
        )
        for (rig in denied) {
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
            assertTrue(rig.port.commands.isEmpty())
            assertTrue(rig.events.isEmpty())
            assertTrue(rig.awake.isEmpty())
        }
    }

    @Test
    fun rejectedLaunchDoesNotReserveTheOwnerAndResolvedPreferencesArePassedUnchanged() {
        val rig = Rig()
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen(GameLaunch(FIXTURE_ID)))
        val runtime = rig.open()
        val start = rig.port.sent<NativeRuntimeCommand.Start>().single()
        assertEquals(runtime.id, start.runtime)
        assertEquals(FIXTURE_ID, start.request.gameId)
        assertEquals(preferences.theme, start.request.preferences.theme)
        assertEquals(preferences.locale, start.request.preferences.locale)
        assertTrue(start.request.preferences.reducedMotion)
        assertTrue(start.foreground)
        assertTrue(rig.ready().isEmpty())
    }

    @Test
    fun nativePreferencesCarryExplicitThemeLocaleAndReducedMotionWithoutDeviceLookup() {
        for (theme in ThemePreference.entries.filter { it != ThemePreference.System }) {
            for (motion in MotionPreference.entries) {
                for (locale in LocalePreference.entries) {
                    val rig = Rig(GameLaunch(FIXTURE_ID, GamePreferences(theme, motion, locale)))
                    rig.open()
                    val facts = rig.port.sent<NativeRuntimeCommand.Start>().single().request.preferences
                    assertEquals(theme, facts.theme)
                    assertEquals(locale, facts.locale)
                    assertEquals(motion == MotionPreference.Reduced, facts.reducedMotion)
                }
            }
        }
    }

    @Test
    fun inventoryAndGrantedCapabilitiesAreDefensiveSnapshots() {
        val ids = mutableSetOf(FIXTURE_ID)
        val supported = mutableSetOf(HostCapability.KeepAwake)
        val granted = mutableSetOf(HostCapability.KeepAwake)
        val rig = Rig(GameLaunch(FIXTURE_ID, preferences, granted), ids, supported)
        ids.clear()
        supported.clear()
        val runtime = rig.open()
        val request = rig.port.sent<NativeRuntimeCommand.Start>().single().request
        granted.clear()
        assertEquals(setOf(HostCapability.KeepAwake), request.capabilities)
        // An immutable implementation may reject this cast/mutation; a mutable snapshot may allow it.
        try {
            @Suppress("UNCHECKED_CAST")
            (request.capabilities as MutableSet<HostCapability>).clear()
        } catch (_: ClassCastException) {
        } catch (_: UnsupportedOperationException) {
        }
        assertEquals(setOf(HostCapability.KeepAwake), request.capabilities)
        val lease = rig.attach(runtime)
        rig.interactive(lease)
        rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
        assertEquals(ReplyCode.Ok, rig.port.sent<NativeRuntimeCommand.ServiceReply>().single().code)
        assertEquals(listOf(true), rig.awake)
    }

    @Test
    fun ownerIsReservedBeforeTheStartPortCanSynchronouslyReenterAdmission() {
        val rig = Rig()
        var reentered: NativeRuntimeOpen? = null
        rig.port.onCommand = { command, _ ->
            if (command is NativeRuntimeCommand.Start) reentered = rig.tryOpen()
        }
        rig.open()
        assertIs<NativeRuntimeOpen.Unavailable>(reentered)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
        assertEquals(1, rig.port.maximumExecutionDepth)
    }

    @Test
    fun everyReadinessFactAndASubsequentInteractiveFrameAreRequired() {
        for (missing in 0..2) {
            val rig = Rig()
            val lease = rig.attach(rig.open())
            val facts = listOf(
                NativeRuntimeEvent.ContextReady(lease),
                NativeRuntimeEvent.ResourcesReady(lease),
                NativeRuntimeEvent.InputReady(lease),
            )
            facts.filterIndexed { index, _ -> index != missing }.forEach(rig.port::emit)
            rig.port.emit(NativeRuntimeEvent.FramePresented(lease, 1))
            assertTrue(rig.ready().isEmpty(), "missing readiness fact $missing")
            rig.port.emit(facts[missing])
            assertTrue(rig.ready().isEmpty(), "a fact alone is not an interactive frame")
            rig.port.emit(NativeRuntimeEvent.FramePresented(lease, 1))
            assertTrue(rig.ready().isEmpty(), "an early frame cannot be replayed")
            rig.now = 145
            rig.port.emit(NativeRuntimeEvent.FramePresented(lease, 2))
            assertEquals(listOf(GameHostEvent.Ready(45)), rig.ready())
            rig.port.emit(NativeRuntimeEvent.FramePresented(lease, 3))
            assertEquals(1, rig.ready().size)
        }
    }

    @Test
    fun frameNumbersMustBePositiveStrictlyIncreasingAndBelongToTheCurrentLease() {
        val rig = Rig()
        val runtime = rig.open()
        val first = rig.attach(runtime)
        rig.port.emit(NativeRuntimeEvent.FramePresented(first, 7))
        val current = assertNotNull(runtime.resize(first, resizedGeometry))
        rig.prerequisites(current)
        rig.port.emit(NativeRuntimeEvent.FramePresented(first, 100))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 0))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, -1))
        assertTrue(rig.ready().isEmpty())
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 1))
        assertEquals(1, rig.ready().size)
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 1))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 0))
        assertEquals(1, rig.ready().size)
    }

    @Test
    fun bootDurationUsesHostClockAndIsClamped() {
        for ((time, expected) in listOf(90L to 0, 100L to 0, 130L to 30, 10_000_000L to 3_600_000)) {
            val rig = Rig()
            val lease = rig.attach(rig.open())
            rig.now = time
            rig.interactive(lease)
            assertEquals(listOf(GameHostEvent.Ready(expected)), rig.ready())
        }
    }

    @Test
    fun recreationAndResizeRetainOneRuntimeAndRetireAllOldReadinessFacts() {
        val rig = Rig()
        val runtime = rig.open()
        val first = rig.attach(runtime)
        rig.prerequisites(first)
        val resized = assertNotNull(runtime.resize(first, resizedGeometry))
        assertNotEquals(first, resized)
        assertEquals(runtime.id, resized.runtime)
        assertFalse(runtime.onBack())
        rig.interactive(first)
        assertTrue(rig.ready().isEmpty())
        rig.port.emit(NativeRuntimeEvent.FramePresented(resized, 1))
        assertTrue(rig.ready().isEmpty(), "resize retires resource/context/input readiness")
        rig.interactive(resized, 2)
        assertEquals(1, rig.ready().size)
        runtime.detach(resized)
        assertNull(runtime.surfaceLease)
        val recreated = rig.attach(runtime)
        assertNotEquals(resized, recreated)
        rig.interactive(resized, 3)
        assertFalse(runtime.onBack())
        rig.interactive(recreated)
        assertTrue(runtime.onBack())
        assertEquals(1, rig.ready().size, "Ready is emitted once per worker, not once per surface")
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Resize>().size)
        assertEquals(2, rig.port.sent<NativeRuntimeCommand.Attach>().size)
    }

    @Test
    fun unchangedGeometryAndStaleDetachResizeInputAreNoOps() {
        val rig = Rig()
        val runtime = rig.open()
        val first = rig.attach(runtime)
        val count = rig.port.commands.size
        assertEquals(first, runtime.resize(first, geometry))
        assertEquals(count, rig.port.commands.size)
        val current = assertNotNull(runtime.resize(first, resizedGeometry))
        rig.interactive(current)
        val afterResize = rig.port.commands.size
        assertEquals(current, runtime.resize(first, geometry))
        runtime.detach(first)
        runtime.cancelPointers(first)
        runtime.pointer(first, NativePointer(0, NativePointer.Phase.Down, 10f, 10f))
        assertEquals(afterResize, rig.port.commands.size)
        assertEquals(current, runtime.surfaceLease)
    }

    @Test
    fun surfaceReferenceLookupReturnsTheFreshLeaseAcrossResizeForegroundAndReplacement() {
        val rig = Rig()
        val runtime = rig.open()
        val firstSurface = object : NativeSurface {}
        val secondSurface = object : NativeSurface {}
        assertNull(runtime.leaseFor(firstSurface))
        val first = assertNotNull(runtime.attach(firstSurface, geometry))
        assertEquals(first, runtime.leaseFor(firstSurface))
        assertNull(runtime.leaseFor(secondSurface))
        val resized = assertNotNull(runtime.resize(first, resizedGeometry))
        assertEquals(resized, runtime.leaseFor(firstSurface))
        runtime.suspend()
        val background = assertNotNull(runtime.leaseFor(firstSurface))
        assertNotEquals(resized, background)
        runtime.resume()
        val foreground = assertNotNull(runtime.leaseFor(firstSurface))
        assertNotEquals(background, foreground)
        val replaced = assertNotNull(runtime.attach(secondSurface, geometry))
        assertNull(runtime.leaseFor(firstSurface))
        assertEquals(replaced, runtime.leaseFor(secondSurface))
        runtime.detach(foreground)
        assertEquals(replaced, runtime.leaseFor(secondSurface))
        runtime.dispose()
        assertNull(runtime.leaseFor(secondSurface))
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
    }

    @Test
    fun geometryRejectsZeroOversizedAndNonfiniteValuesBeforeAPlatformAttach() {
        for ((width, height) in listOf(0 to 1, 1 to 0, -1 to 1, 16_385 to 1, 1 to 16_385)) {
            assertNull(NativeSurfaceGeometry.create(width, height, 1f))
        }
        for (density in listOf(0f, 0.49f, 16.01f, Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY)) {
            assertNull(NativeSurfaceGeometry.create(1, 1, density))
        }
        assertNotNull(NativeSurfaceGeometry.create(1, 1, 0.5f))
        assertNotNull(NativeSurfaceGeometry.create(16_384, 16_384, 16f))
    }

    @Test
    fun initialBackgroundCannotRenderOrBecomeReadyAndResumeNeedsFreshInputAndFrame() {
        val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
        val runtime = rig.open(foreground = false)
        assertFalse(rig.port.sent<NativeRuntimeCommand.Start>().single().foreground)
        val lease = rig.attach(runtime)
        assertEquals(listOf(false), rig.port.sent<NativeRuntimeCommand.Rendering>().map { it.enabled })
        rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
        assertEquals(ReplyCode.Unsupported, rig.port.sent<NativeRuntimeCommand.ServiceReply>().single().code)
        assertTrue(rig.awake.isEmpty())
        rig.interactive(lease)
        assertTrue(rig.ready().isEmpty())
        assertFalse(runtime.onBack())
        runtime.resume()
        val current = assertNotNull(runtime.surfaceLease)
        assertNotEquals(lease, current)
        rig.interactive(lease, 10)
        rig.port.emit(NativeRuntimeEvent.ContextReady(current))
        rig.port.emit(NativeRuntimeEvent.ResourcesReady(current))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 2))
        assertTrue(rig.ready().isEmpty(), "background InputReady is not reusable")
        rig.port.emit(NativeRuntimeEvent.InputReady(current))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 2))
        assertTrue(rig.ready().isEmpty(), "the pre-input frame is not reusable")
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 3))
        assertEquals(1, rig.ready().size)
        assertTrue(runtime.onBack())
    }

    @Test
    fun suspendResumeRetainsMatchButRequiresFreshInputAndFrameBeforeInteraction() {
        val rig = Rig()
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        rig.interactive(lease)
        runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 10f, 20f))
        runtime.suspend()
        val commandsAfterSuspend = rig.port.commands.size
        runtime.suspend()
        assertEquals(commandsAfterSuspend, rig.port.commands.size)
        assertFalse(runtime.onBack())
        runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Move, 11f, 20f))
        val background = assertNotNull(runtime.surfaceLease)
        assertNotEquals(lease, background)
        rig.interactive(background, 2)
        runtime.resume()
        val current = assertNotNull(runtime.surfaceLease)
        assertNotEquals(background, current)
        val commandsAfterResume = rig.port.commands.size
        runtime.resume()
        assertEquals(commandsAfterResume, rig.port.commands.size)
        rig.interactive(lease, 20)
        rig.interactive(background, 20)
        rig.port.emit(NativeRuntimeEvent.ContextReady(current))
        rig.port.emit(NativeRuntimeEvent.ResourcesReady(current))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 3))
        assertFalse(runtime.onBack())
        rig.port.emit(NativeRuntimeEvent.InputReady(current))
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 3))
        assertFalse(runtime.onBack())
        rig.port.emit(NativeRuntimeEvent.FramePresented(current, 4))
        assertTrue(runtime.onBack())
        runtime.pointer(current, NativePointer(0, NativePointer.Phase.Move, 12f, 20f))
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Pointer>().size, "suspend cancels existing drags")
        assertEquals(listOf(false, true), rig.port.sent<NativeRuntimeCommand.Foreground>().map { it.active })
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
        assertEquals(1, rig.ready().size)
    }

    @Test
    fun inputIsBlockedWhileLoadingAndTracksOnlyActivePointers() {
        val rig = Rig()
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 1f, 2f))
        assertTrue(rig.port.sent<NativeRuntimeCommand.Pointer>().isEmpty())
        rig.interactive(lease)
        runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Move, 1f, 2f))
        runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Up, 1f, 2f))
        assertTrue(rig.port.sent<NativeRuntimeCommand.Pointer>().isEmpty())
        runtime.pointer(lease, NativePointer(31, NativePointer.Phase.Down, -32_768f, 32_768f))
        runtime.pointer(lease, NativePointer(31, NativePointer.Phase.Move, 2f, 3f))
        runtime.pointer(lease, NativePointer(31, NativePointer.Phase.Up, 2f, 3f))
        runtime.pointer(lease, NativePointer(31, NativePointer.Phase.Move, 3f, 4f))
        assertEquals(listOf(NativePointer.Phase.Down, NativePointer.Phase.Move, NativePointer.Phase.Up), rig.port.sent<NativeRuntimeCommand.Pointer>().map { it.input.phase })
    }

    @Test
    fun malformedDuplicateAndEleventhPointerCancelWithoutForwardingHostileInput() {
        val hostile = listOf(
            NativePointer(-1, NativePointer.Phase.Down, 0f, 0f),
            NativePointer(32, NativePointer.Phase.Down, 0f, 0f),
            NativePointer(1, NativePointer.Phase.Down, Float.NaN, 0f),
            NativePointer(1, NativePointer.Phase.Down, 0f, Float.POSITIVE_INFINITY),
            NativePointer(1, NativePointer.Phase.Down, -32_769f, 0f),
            NativePointer(1, NativePointer.Phase.Down, 0f, 32_769f),
            NativePointer(0, NativePointer.Phase.Down, 0f, 0f),
        )
        for (input in hostile) {
            val rig = Rig()
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 0f, 0f))
            runtime.pointer(lease, input)
            runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Move, 1f, 1f))
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Pointer>().size, input.toString())
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.CancelPointers>().size, input.toString())
        }
        val rig = Rig()
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        rig.interactive(lease)
        for (id in 0..10) runtime.pointer(lease, NativePointer(id, NativePointer.Phase.Down, 0f, 0f))
        assertEquals(10, rig.port.sent<NativeRuntimeCommand.Pointer>().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.CancelPointers>().size)
    }

    @Test
    fun surfaceLossSuspendFailureAndDisposeCancelPointersBeforeRetiringWork() {
        for (loss in listOf("detach", "suspend", "failure", "dispose", "resize")) {
            val rig = Rig()
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 0f, 0f))
            val before = rig.port.commands.size
            when (loss) {
                "detach" -> runtime.detach(lease)
                "suspend" -> runtime.suspend()
                "failure" -> rig.port.emit(NativeRuntimeEvent.Failed(runtime.id, NativeRuntimeEvent.Failure.GraphicsContext))
                "dispose" -> runtime.dispose()
                "resize" -> runtime.resize(lease, resizedGeometry)
            }
            val cleanup = rig.port.commands.drop(before)
            assertIs<NativeRuntimeCommand.CancelPointers>(cleanup.first(), loss)
            assertEquals(lease, (cleanup.first() as NativeRuntimeCommand.CancelPointers).lease, loss)
            runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Move, 1f, 1f))
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Pointer>().size, loss)
        }
    }

    @Test
    fun keepAwakeIsDeniedWithoutGrantAndUnsupportedBeforeInteractive() {
        val denied = Rig()
        val deniedLease = denied.attach(denied.open())
        denied.interactive(deniedLease)
        denied.port.emit(NativeRuntimeEvent.KeepAwake(deniedLease, 1, true))
        assertEquals(ReplyCode.Denied, denied.port.sent<NativeRuntimeCommand.ServiceReply>().single().code)
        assertTrue(denied.awake.isEmpty())

        val allowed = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
        val runtime = allowed.open()
        val lease = allowed.attach(runtime)
        allowed.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
        assertEquals(ReplyCode.Unsupported, allowed.port.sent<NativeRuntimeCommand.ServiceReply>().single().code)
        assertTrue(allowed.awake.isEmpty())
        allowed.interactive(lease)
        allowed.port.emit(NativeRuntimeEvent.KeepAwake(lease, 2, true))
        runtime.suspend()
        allowed.port.emit(NativeRuntimeEvent.KeepAwake(assertNotNull(runtime.surfaceLease), 3, true))
        assertEquals(listOf(ReplyCode.Unsupported, ReplyCode.Ok, ReplyCode.Unsupported), allowed.port.sent<NativeRuntimeCommand.ServiceReply>().map { it.code })
        assertEquals(listOf(true, false), allowed.awake)
    }

    @Test
    fun keepAwakeIdsArePositiveMonotonicAndCannotBeReplayedAcrossSurfaceLeases() {
        val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
        val runtime = rig.open()
        val first = rig.attach(runtime)
        rig.interactive(first)
        for (id in listOf(-1, 0)) rig.port.emit(NativeRuntimeEvent.KeepAwake(first, id, true))
        rig.port.emit(NativeRuntimeEvent.KeepAwake(first, 4, true))
        for (id in listOf(4, 3, 1)) rig.port.emit(NativeRuntimeEvent.KeepAwake(first, id, false))
        assertEquals(listOf(4), rig.port.sent<NativeRuntimeCommand.ServiceReply>().map { it.requestId })
        assertEquals(listOf(true), rig.awake)
        val current = assertNotNull(runtime.resize(first, resizedGeometry))
        rig.interactive(current)
        rig.port.emit(NativeRuntimeEvent.KeepAwake(first, 10, true))
        rig.port.emit(NativeRuntimeEvent.KeepAwake(current, 4, true))
        rig.port.emit(NativeRuntimeEvent.KeepAwake(current, 5, false))
        assertEquals(listOf(4, 5), rig.port.sent<NativeRuntimeCommand.ServiceReply>().map { it.requestId })
        assertEquals(ReplyCode.Ok, rig.port.sent<NativeRuntimeCommand.ServiceReply>().last().code)
    }

    @Test
    fun keepAwakeIsReleasedOnEverySurfaceOrWorkerRetirementAndNeverAutomaticallyRestored() {
        for (loss in listOf("detach", "suspend", "failure", "dispose", "resize", "replace")) {
            val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
            when (loss) {
                "detach" -> runtime.detach(lease)
                "suspend" -> runtime.suspend()
                "failure" -> rig.port.emit(NativeRuntimeEvent.Failed(runtime.id, NativeRuntimeEvent.Failure.Resources))
                "dispose" -> runtime.dispose()
                "resize" -> runtime.resize(lease, resizedGeometry)
                "replace" -> rig.attach(runtime)
            }
            assertEquals(listOf(true, false), rig.awake, loss)
            runtime.resume()
            assertEquals(listOf(true, false), rig.awake, "$loss must not automatically restore the service")
        }
    }

    @Test
    fun backIsUnconsumedWhileLoadingAndInteractiveExitStopsThenNotifiesOnce() {
        val rig = Rig()
        val runtime = rig.open()
        assertFalse(runtime.onBack())
        val lease = rig.attach(runtime)
        rig.prerequisites(lease)
        assertFalse(runtime.onBack())
        rig.port.emit(NativeRuntimeEvent.FramePresented(lease, 1))
        assertTrue(runtime.onBack())
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Back>().size)
        assertFalse(GameHostEvent.Exited in rig.events, "Back alone cannot imply exit confirmation")
        rig.port.emit(NativeRuntimeEvent.ExitConfirmed(runtime.id))
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertEquals(1, rig.events.count { it == GameHostEvent.Exited })
        assertFalse(runtime.onBack())
        rig.port.emit(NativeRuntimeEvent.ExitConfirmed(runtime.id))
        rig.port.emit(NativeRuntimeEvent.Failed(runtime.id, NativeRuntimeEvent.Failure.Runtime))
        assertEquals(1, rig.events.count { it == GameHostEvent.Exited })
        assertTrue(rig.failures().isEmpty())
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
        rig.open()
    }

    @Test
    fun disposalBeforeAnySurfaceIsIdempotentAndDelayedStoppedBlocksReopen() {
        val rig = Rig()
        val runtime = rig.open()
        runtime.dispose()
        runtime.dispose()
        runtime.suspend()
        runtime.resume()
        assertNull(runtime.attach(object : NativeSurface {}, geometry))
        assertFalse(runtime.onBack())
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertTrue(rig.port.sent<NativeRuntimeCommand.Attach>().isEmpty())
        assertTrue(rig.events.isEmpty())
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        rig.port.emit(NativeRuntimeEvent.Stopped(NativeRuntimeId(runtime.id.value + 1)))
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
        val newer = rig.open()
        assertNotEquals(runtime.id, newer.id)
        assertEquals(2, rig.port.sent<NativeRuntimeCommand.Start>().size)
    }

    @Test
    fun unsolicitedAndStaleStoppedCannotReleaseALiveOrNewerWorker() {
        val rig = Rig()
        val first = rig.open()
        val oldCallback = rig.port.callbackFor(first.id)
        rig.port.emit(NativeRuntimeEvent.Stopped(first.id))
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        first.dispose()
        rig.port.emit(NativeRuntimeEvent.Stopped(first.id))
        val current = rig.open()
        rig.port.emit(NativeRuntimeEvent.Stopped(first.id))
        oldCallback(NativeRuntimeEvent.Stopped(first.id))
        oldCallback(NativeRuntimeEvent.Stopped(current.id))
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        current.dispose()
        rig.port.emit(NativeRuntimeEvent.Stopped(first.id))
        oldCallback(NativeRuntimeEvent.Stopped(first.id))
        oldCallback(NativeRuntimeEvent.Stopped(current.id))
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        rig.port.emit(NativeRuntimeEvent.Stopped(current.id))
        rig.open()
    }

    @Test
    fun retiredSurfaceAndRuntimeCallbacksCannotResurrectReadinessServicesOrExit() {
        val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
        val first = rig.open()
        val oldLease = rig.attach(first)
        first.dispose()
        rig.interactive(oldLease)
        rig.port.emit(NativeRuntimeEvent.KeepAwake(oldLease, 1, true))
        rig.port.emit(NativeRuntimeEvent.ExitConfirmed(first.id))
        assertTrue(rig.events.isEmpty())
        assertTrue(rig.awake.isEmpty())
        rig.port.emit(NativeRuntimeEvent.Stopped(first.id))
        val current = rig.open()
        val currentLease = rig.attach(current)
        rig.interactive(oldLease)
        rig.port.emit(NativeRuntimeEvent.KeepAwake(oldLease, 2, true))
        rig.port.emit(NativeRuntimeEvent.Failed(first.id, NativeRuntimeEvent.Failure.Runtime))
        rig.port.emit(NativeRuntimeEvent.ExitConfirmed(first.id))
        assertTrue(rig.events.isEmpty())
        rig.interactive(currentLease)
        assertEquals(1, rig.ready().size)
    }

    @Test
    fun repeatedOpenCloseHasOneStartStopPerWorkerAndNoCurrentSurfaceAfterClose() {
        val rig = Rig()
        var previous = 0L
        repeat(20) {
            val runtime = rig.open()
            assertTrue(runtime.id.value > previous)
            previous = runtime.id.value
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            runtime.dispose()
            assertNull(runtime.surfaceLease)
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
            rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
        }
        assertEquals(20, rig.port.sent<NativeRuntimeCommand.Start>().size)
        assertEquals(20, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertEquals(20, rig.port.sent<NativeRuntimeCommand.Detach>().size)
        assertEquals(20, rig.ready().size)
    }

    @Test
    fun backendFailuresAreExplicitAndAlwaysWaitForTheMatchingStopAcknowledgement() {
        for (code in NativeRuntimeEvent.Failure.entries) {
            val rig = Rig()
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            rig.port.emit(NativeRuntimeEvent.Failed(runtime.id, code))
            rig.port.emit(NativeRuntimeEvent.Failed(runtime.id, code))
            assertEquals(listOf(GameHostEvent.Failed("native-${code.name.lowercase()}", false)), rig.failures())
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
            assertNull(runtime.surfaceLease)
            assertFalse(runtime.onBack())
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
            rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
            rig.open()
        }
    }

    @Test
    fun startDriverExceptionIsReportedAndAThrownStopCannotPretendTheWorkerJoined() {
        val rig = Rig()
        rig.port.onCommand = { command, _ ->
            if (command is NativeRuntimeCommand.Start || command is NativeRuntimeCommand.Stop) error("driver failed")
        }
        val runtime = rig.open()
        assertEquals(listOf(GameHostEvent.Failed("native-driver", false)), rig.failures())
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        rig.port.onCommand = { _, _ -> }
        rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
        rig.open()
    }

    @Test
    fun attachDriverExceptionRetiresQueuedRenderingEnableBeforeTeardown() {
        val rig = Rig()
        val runtime = rig.open()
        rig.port.onCommand = { command, _ -> if (command is NativeRuntimeCommand.Attach) error("attach failed") }
        assertNull(runtime.attach(object : NativeSurface {}, geometry))
        assertEquals(listOf(GameHostEvent.Failed("native-driver", false)), rig.failures())
        assertTrue(rig.port.sent<NativeRuntimeCommand.Rendering>().none { it.enabled })
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
    }

    @Test
    fun attachDriverExceptionFencesExactlyItsSurfaceBeforeTheHostCanRecoverOrDispose() {
        val rig = Rig()
        val runtime = rig.open()
        var attached: NativeSurfaceLease? = null
        val order = mutableListOf<String>()
        rig.port.onCommand = { command, _ ->
            if (command is NativeRuntimeCommand.Attach) {
                attached = command.lease
                error("Attach driver failed")
            }
        }
        rig.port.onFence = { lease ->
            assertEquals(attached, lease)
            order += "fence"
        }
        rig.onEvent = { event ->
            if (event is GameHostEvent.Failed) {
                assertEquals(listOf(assertNotNull(attached)), rig.port.fences)
                order += "host-recovery"
                runtime.dispose()
            }
        }
        assertNull(runtime.attach(object : NativeSurface {}, geometry))
        assertEquals(listOf(assertNotNull(attached)), rig.port.fences)
        assertEquals(listOf("fence", "host-recovery"), order)
        assertEquals(listOf(GameHostEvent.Failed("native-driver", false)), rig.failures())
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Detach>().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertTrue(rig.port.sent<NativeRuntimeCommand.Rendering>().none { it.enabled })
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
    }

    @Test
    fun driverExceptionsInInputServicesAndLifecycleFailOnceAndRetainAdmissionUntilJoined() {
        for (operation in listOf("resize", "pointer", "foreground", "back", "service", "cancel", "detach", "rendering")) {
            val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            var threw = false
            rig.port.onCommand = { command, _ ->
                val target = when (operation) {
                    "resize" -> command is NativeRuntimeCommand.Resize
                    "pointer" -> command is NativeRuntimeCommand.Pointer
                    "foreground" -> command is NativeRuntimeCommand.Foreground
                    "back" -> command is NativeRuntimeCommand.Back
                    "service" -> command is NativeRuntimeCommand.ServiceReply
                    "cancel" -> command is NativeRuntimeCommand.CancelPointers
                    "detach" -> command is NativeRuntimeCommand.Detach
                    else -> command is NativeRuntimeCommand.Rendering
                }
                if (target && !threw) {
                    threw = true
                    error("controlled $operation driver exception")
                }
            }
            when (operation) {
                "resize" -> runtime.resize(lease, resizedGeometry)
                "pointer" -> runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 0f, 0f))
                "foreground", "rendering" -> runtime.suspend()
                "back" -> runtime.onBack()
                "service" -> rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
                "cancel" -> runtime.cancelPointers(lease)
                "detach" -> runtime.detach(lease)
            }
            assertTrue(threw, "$operation exception path must be reached")
            assertEquals(listOf(GameHostEvent.Failed("native-driver", false)), rig.failures(), operation)
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size, operation)
            assertNull(runtime.surfaceLease, operation)
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen(), operation)
            rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
            rig.open()
        }
    }

    @Test
    fun synchronousAttachFailureOrExitCannotEnableARetiredSurface() {
        for (exit in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            rig.port.onCommand = { command, emit ->
                if (command is NativeRuntimeCommand.Attach) {
                    emit(if (exit) NativeRuntimeEvent.ExitConfirmed(command.runtime) else NativeRuntimeEvent.Failed(command.runtime, NativeRuntimeEvent.Failure.Input))
                }
            }
            assertNull(runtime.attach(object : NativeSurface {}, geometry))
            assertTrue(rig.port.sent<NativeRuntimeCommand.Rendering>().none { it.enabled }, "exit=$exit")
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
            assertEquals(1, rig.port.maximumExecutionDepth)
            assertEquals(1, rig.events.size)
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        }
    }

    @Test
    fun detachAndDisposeDuringAttachFenceTheSurfaceBeforeTheReentrantCallReturns() {
        for (dispose in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            var verifiedBeforeAttachReturned = false
            rig.port.onCommand = { command, _ ->
                if (command is NativeRuntimeCommand.Attach) {
                    if (dispose) runtime.dispose() else runtime.detach(command.lease)
                    assertEquals(listOf(command.lease), rig.port.fences, "dispose=$dispose")
                    assertTrue(rig.port.sent<NativeRuntimeCommand.Detach>().isEmpty(), "the immediate fence precedes deferred cleanup")
                    verifiedBeforeAttachReturned = true
                }
            }
            assertNull(runtime.attach(object : NativeSurface {}, geometry))
            assertTrue(verifiedBeforeAttachReturned)
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Detach>().size)
            assertEquals(if (dispose) 1 else 0, rig.port.sent<NativeRuntimeCommand.Stop>().size)
            assertTrue(rig.port.sent<NativeRuntimeCommand.Rendering>().none { it.enabled })
            assertEquals(1, rig.port.maximumExecutionDepth)
            assertTrue(rig.events.isEmpty())
        }
    }

    @Test
    fun throwingSurfaceFenceFailsClosedAndCannotEnableAReplacementSurface() {
        for (replace in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            val original = rig.attach(runtime)
            rig.interactive(original)
            rig.port.onFence = { error("surface fence failed") }
            val before = rig.port.commands.size
            if (replace) {
                assertNull(runtime.attach(object : NativeSurface {}, resizedGeometry))
            } else {
                runtime.detach(original)
            }
            assertEquals(listOf(GameHostEvent.Failed("native-graphicscontext", false)), rig.failures(), "replace=$replace")
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
            assertTrue(rig.port.commands.drop(before).filterIsInstance<NativeRuntimeCommand.Rendering>().none { it.enabled })
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Attach>().size)
            assertNull(runtime.surfaceLease)
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
            rig.port.onFence = {}
            rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
            rig.open()
        }
    }

    @Test
    fun synchronousFrameThenRetirementFencesQueuedReadyAndKeepAwakeEnablingEffects() {
        for (failure in listOf(false, true)) {
            val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
            val runtime = rig.open()
            rig.port.onCommand = { command, emit ->
                if (command is NativeRuntimeCommand.Attach) {
                    emit(NativeRuntimeEvent.ContextReady(command.lease))
                    emit(NativeRuntimeEvent.ResourcesReady(command.lease))
                    emit(NativeRuntimeEvent.InputReady(command.lease))
                    emit(NativeRuntimeEvent.FramePresented(command.lease, 1))
                    emit(NativeRuntimeEvent.KeepAwake(command.lease, 1, true))
                    if (failure) {
                        emit(NativeRuntimeEvent.Failed(command.runtime, NativeRuntimeEvent.Failure.Resources))
                    } else {
                        runtime.dispose()
                    }
                }
            }
            assertNull(runtime.attach(object : NativeSurface {}, geometry))
            assertTrue(rig.ready().isEmpty(), "failure=$failure: a retired surface cannot notify Ready")
            assertTrue(rig.awake.none { it }, "failure=$failure: a retired surface cannot enable keepAwake")
            assertTrue(rig.port.sent<NativeRuntimeCommand.Rendering>().none { it.enabled })
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
            assertEquals(if (failure) 1 else 0, rig.failures().size)
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        }
    }

    @Test
    fun disposalDuringAttachSuppressesAlreadyQueuedFailureAndExitNotifications() {
        for (exit in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            rig.port.onCommand = { command, emit ->
                if (command is NativeRuntimeCommand.Attach) {
                    emit(if (exit) NativeRuntimeEvent.ExitConfirmed(command.runtime) else NativeRuntimeEvent.Failed(command.runtime, NativeRuntimeEvent.Failure.Runtime))
                    runtime.dispose()
                }
            }
            assertNull(runtime.attach(object : NativeSurface {}, geometry))
            assertTrue(rig.events.isEmpty(), "exit=$exit: disposed host callbacks are retired")
            assertEquals(1, rig.port.fences.size)
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
            assertTrue(rig.port.sent<NativeRuntimeCommand.Rendering>().none { it.enabled })
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
            rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
            rig.port.onCommand = { _, _ -> }
            rig.open()
        }
    }

    @Test
    fun synchronousReadinessAndReentrantDisposeDrainWithoutNestedPortExecution() {
        val rig = Rig()
        val runtime = rig.open()
        rig.onEvent = { if (it is GameHostEvent.Ready) runtime.dispose() }
        rig.port.onCommand = { command, emit ->
            if (command is NativeRuntimeCommand.Attach) {
                emit(NativeRuntimeEvent.ContextReady(command.lease))
                emit(NativeRuntimeEvent.ResourcesReady(command.lease))
                emit(NativeRuntimeEvent.InputReady(command.lease))
                emit(NativeRuntimeEvent.FramePresented(command.lease, 1))
            }
            if (command is NativeRuntimeCommand.Stop) emit(NativeRuntimeEvent.Stopped(command.runtime))
        }
        assertNull(runtime.attach(object : NativeSurface {}, geometry))
        assertEquals(1, rig.ready().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertEquals(1, rig.port.maximumExecutionDepth)
        assertNull(runtime.surfaceLease)
        rig.port.onCommand = { _, _ -> }
        rig.open()
    }

    @Test
    fun reentrantResizeDuringAttachFencesThePreviousLeaseAndItsQueuedRenderingEnable() {
        val rig = Rig()
        val runtime = rig.open()
        var retired: NativeSurfaceLease? = null
        rig.port.onCommand = { command, emit ->
            if (command is NativeRuntimeCommand.Attach) {
                retired = command.lease
                runtime.resize(command.lease, resizedGeometry)
                emit(NativeRuntimeEvent.ContextReady(command.lease))
                emit(NativeRuntimeEvent.ResourcesReady(command.lease))
                emit(NativeRuntimeEvent.InputReady(command.lease))
                emit(NativeRuntimeEvent.FramePresented(command.lease, 1))
            }
        }
        val current = assertNotNull(runtime.attach(object : NativeSurface {}, geometry))
        assertNotEquals(retired, current)
        assertTrue(rig.ready().isEmpty())
        assertEquals(listOf(current), rig.port.sent<NativeRuntimeCommand.Rendering>().filter { it.enabled }.map { it.lease })
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Resize>().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
        assertEquals(1, rig.port.maximumExecutionDepth)
        rig.interactive(current)
        assertEquals(1, rig.ready().size)
    }

    @Test
    fun reentrantResizeBeforeQueuedAttachDispatchStillAttachesTheReplacementSurface() {
        val rig = Rig()
        val runtime = rig.open()
        val original = rig.attach(runtime)
        rig.interactive(original)
        val replacement = object : NativeSurface {}
        val before = rig.port.commands.size
        rig.port.onCommand = { command, _ ->
            if (command is NativeRuntimeCommand.Back) {
                val queued = assertNotNull(runtime.attach(replacement, geometry))
                runtime.resize(queued, resizedGeometry)
            }
        }

        assertTrue(runtime.onBack())
        val current = assertNotNull(runtime.leaseFor(replacement))
        val sent = rig.port.commands.drop(before)
        val attached = sent.filterIsInstance<NativeRuntimeCommand.Attach>().single()
        assertTrue(attached.surface === replacement)
        assertEquals(current, attached.lease)
        assertEquals(resizedGeometry, attached.geometry)
        val attachIndex = sent.indexOf(attached)
        assertTrue(sent.take(attachIndex).none { it is NativeRuntimeCommand.Resize }, "a replacement must be attached before it can be resized")
        assertTrue(sent.take(attachIndex).filterIsInstance<NativeRuntimeCommand.Rendering>().none { it.enabled }, "rendering cannot start before the replacement is attached")
        assertEquals(listOf(current), sent.filterIsInstance<NativeRuntimeCommand.Rendering>().filter { it.enabled }.map { it.lease })
        assertEquals(listOf(original), rig.port.fences)
        assertFalse(runtime.onBack(), "the replacement still needs fresh readiness")
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
    }

    @Test
    fun reentrantForegroundChangesBeforeQueuedAttachDispatchStillAttachItsSurface() {
        for (resume in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            val original = rig.attach(runtime)
            rig.interactive(original)
            val replacement = object : NativeSurface {}
            val before = rig.port.commands.size
            rig.port.onCommand = { command, _ ->
                if (command is NativeRuntimeCommand.Back) {
                    runtime.attach(replacement, geometry)
                    runtime.suspend()
                    if (resume) runtime.resume()
                }
            }

            assertTrue(runtime.onBack())
            val current = assertNotNull(runtime.leaseFor(replacement))
            val sent = rig.port.commands.drop(before)
            val attached = sent.filterIsInstance<NativeRuntimeCommand.Attach>().single()
            assertTrue(attached.surface === replacement, "resume=$resume")
            assertEquals(current, attached.lease, "resume=$resume")
            assertEquals(geometry, attached.geometry, "resume=$resume")
            val attachIndex = sent.indexOf(attached)
            assertTrue(sent.take(attachIndex).none { it is NativeRuntimeCommand.Resize }, "resume=$resume: a foreground change cannot configure an unattached replacement")
            assertTrue(sent.take(attachIndex).filterIsInstance<NativeRuntimeCommand.Rendering>().none { it.enabled }, "resume=$resume: rendering needs the attached surface")
            assertEquals(if (resume) listOf(current) else emptyList(), sent.filterIsInstance<NativeRuntimeCommand.Rendering>().filter { it.enabled }.map { it.lease })
            assertEquals(listOf(original), rig.port.fences, "resume=$resume")
            assertFalse(runtime.onBack(), "resume=$resume: the replacement still needs fresh readiness")
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size, "resume=$resume")
        }
    }

    @Test
    fun reentrantRetirementBeforeQueuedResizeDispatchFencesOnlyTheDispatchedSurfaceLease() {
        for (dispose in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            val original = rig.attach(runtime)
            rig.interactive(original)
            val before = rig.port.commands.size
            rig.port.onCommand = { command, _ ->
                if (command is NativeRuntimeCommand.Back) {
                    val pending = assertNotNull(runtime.resize(original, resizedGeometry))
                    assertNotEquals(original, pending)
                    assertTrue(rig.port.sent<NativeRuntimeCommand.Resize>().isEmpty())
                    if (dispose) runtime.dispose() else runtime.detach(pending)
                    assertEquals(listOf(original), rig.port.fences, "the fence must name the surface ticket actually known to the backend")
                }
            }

            assertTrue(runtime.onBack())
            val sent = rig.port.commands.drop(before)
            assertTrue(sent.filterIsInstance<NativeRuntimeCommand.Resize>().isEmpty(), "dispose=$dispose: the retired resize must never reach the backend")
            assertTrue(sent.filterIsInstance<NativeRuntimeCommand.Rendering>().none { it.enabled }, "dispose=$dispose")
            assertEquals(listOf(original), sent.filterIsInstance<NativeRuntimeCommand.Detach>().map { it.lease }, "dispose=$dispose")
            assertEquals(if (dispose) 1 else 0, rig.port.sent<NativeRuntimeCommand.Stop>().size, "dispose=$dispose")
            assertNull(runtime.surfaceLease, "dispose=$dispose")
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Attach>().size, "dispose=$dispose")
        }
    }

    @Test
    fun reentrantDetachBeforeQueuedAttachDispatchDoesNotFenceAnUndispatchedReplacement() {
        val rig = Rig()
        val runtime = rig.open()
        val original = rig.attach(runtime)
        rig.interactive(original)
        val replacement = object : NativeSurface {}
        val before = rig.port.commands.size
        rig.port.onCommand = { command, _ ->
            if (command is NativeRuntimeCommand.Back) {
                val queued = assertNotNull(runtime.attach(replacement, geometry))
                val pending = assertNotNull(runtime.resize(queued, resizedGeometry))
                runtime.detach(pending)
                assertEquals(listOf(original), rig.port.fences, "the replacement never reached the backend and needs no native surface fence")
            }
        }

        assertTrue(runtime.onBack())
        val sent = rig.port.commands.drop(before)
        assertTrue(sent.filterIsInstance<NativeRuntimeCommand.Attach>().isEmpty())
        assertTrue(sent.filterIsInstance<NativeRuntimeCommand.Resize>().isEmpty())
        assertTrue(sent.filterIsInstance<NativeRuntimeCommand.Rendering>().none { it.enabled })
        assertEquals(listOf(original), sent.filterIsInstance<NativeRuntimeCommand.Detach>().map { it.lease })
        assertNull(runtime.surfaceLease)
        assertNull(runtime.leaseFor(replacement))
        assertTrue(rig.port.sent<NativeRuntimeCommand.Stop>().isEmpty())
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Start>().size)
    }

    @Test
    fun reentrantSurfaceFenceCannotDispatchAReplacementBeforeThePreviousDetach() {
        val rig = Rig()
        val runtime = rig.open()
        val original = rig.attach(runtime)
        rig.interactive(original)
        val replacement = object : NativeSurface {}
        val before = rig.port.commands.size
        var reentered = false
        rig.port.onFence = { fenced ->
            assertEquals(original, fenced)
            assertFalse(reentered)
            reentered = true
            val pending = assertNotNull(runtime.leaseFor(replacement))
            runtime.resize(pending, resizedGeometry)
            assertTrue(rig.port.commands.drop(before).none { it is NativeRuntimeCommand.Attach }, "the replacement cannot execute until the immediate fence returns")
        }
        rig.port.onCommand = { command, _ ->
            if (command is NativeRuntimeCommand.Back) runtime.attach(replacement, geometry)
        }

        assertTrue(runtime.onBack())
        assertTrue(reentered, "the nested resize must be reached during the immediate fence")
        val current = assertNotNull(runtime.leaseFor(replacement))
        val sent = rig.port.commands.drop(before)
        val detached = sent.filterIsInstance<NativeRuntimeCommand.Detach>().single()
        val attached = sent.filterIsInstance<NativeRuntimeCommand.Attach>().single()
        val enabled = sent.filterIsInstance<NativeRuntimeCommand.Rendering>().single { it.enabled }
        assertEquals(original, detached.lease)
        assertEquals(current, attached.lease)
        assertTrue(attached.surface === replacement)
        assertEquals(resizedGeometry, attached.geometry)
        assertEquals(current, enabled.lease)
        assertTrue(sent.indexOf(detached) < sent.indexOf(attached), "retiring the previous native surface must precede replacement attachment")
        assertTrue(sent.indexOf(attached) < sent.indexOf(enabled), "replacement rendering must follow its attachment")
        assertEquals(listOf(original), rig.port.fences)
        assertEquals(1, rig.port.maximumExecutionDepth)
    }

    @Test
    fun synchronousStartFailureCanCompleteTeardownBeforeOpenReturnsWithoutLeakingAdmission() {
        val rig = Rig()
        rig.port.onCommand = { command, emit ->
            if (command is NativeRuntimeCommand.Start) emit(NativeRuntimeEvent.Failed(command.runtime, NativeRuntimeEvent.Failure.Runtime))
            if (command is NativeRuntimeCommand.Stop) emit(NativeRuntimeEvent.Stopped(command.runtime))
        }
        val retired = rig.open()
        assertNull(retired.surfaceLease)
        assertFalse(retired.onBack())
        assertEquals(listOf(GameHostEvent.Failed("native-runtime", false)), rig.failures())
        assertEquals(1, rig.port.maximumExecutionDepth)
        rig.port.onCommand = { _, _ -> }
        assertNotEquals(retired.id, rig.open().id)
    }

    @Test
    fun synchronousStopAcknowledgementReleasesOnlyAfterStopAndReentrantNotificationIsSafe() {
        val rig = Rig()
        val runtime = rig.open()
        var duringFailure: NativeRuntimeOpen? = null
        rig.onEvent = { if (it is GameHostEvent.Failed) duringFailure = rig.tryOpen() }
        rig.port.onCommand = { command, emit ->
            if (command is NativeRuntimeCommand.Stop) emit(NativeRuntimeEvent.Stopped(command.runtime))
        }
        rig.port.emit(NativeRuntimeEvent.Failed(runtime.id, NativeRuntimeEvent.Failure.Runtime))
        assertIs<NativeRuntimeOpen.Unavailable>(duringFailure)
        assertEquals(1, rig.failures().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        rig.open()
    }

    @Test
    fun aReentrantServiceCallbackCannotLeaveKeepAwakeEnabledAfterDispose() {
        val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        rig.interactive(lease)
        rig.onAwake = { if (it) runtime.dispose() }
        rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
        assertEquals(listOf(true, false), rig.awake)
        assertNull(runtime.surfaceLease)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
    }

    @Test
    fun throwingKeepAwakeReleaseCallbackCannotStrandPointerSurfaceOrWorkerCleanup() {
        val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        rig.interactive(lease)
        runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 0f, 0f))
        rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
        rig.onAwake = { if (!it) error("release callback failed") }
        rig.port.onCommand = { command, emit ->
            if (command is NativeRuntimeCommand.Stop) emit(NativeRuntimeEvent.Stopped(command.runtime))
        }
        // Whether the observer exception is surfaced is independent of completing native cleanup.
        try {
            runtime.dispose()
        } catch (_: IllegalStateException) {
        }
        assertEquals(listOf(true, false), rig.awake)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.CancelPointers>().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Detach>().size)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertNull(runtime.surfaceLease)
        rig.onAwake = {}
        rig.port.onCommand = { _, _ -> }
        rig.open()
    }

    @Test
    fun throwingFailureOrExitObserverCannotStrandTheMatchingQueuedOwnerRelease() {
        for (exit in listOf(false, true)) {
            val rig = Rig()
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.interactive(lease)
            rig.onEvent = { if (it is GameHostEvent.Failed || it == GameHostEvent.Exited) error("observer failed") }
            rig.port.onCommand = { command, emit ->
                if (command is NativeRuntimeCommand.Stop) emit(NativeRuntimeEvent.Stopped(command.runtime))
            }
            try {
                rig.port.emit(if (exit) NativeRuntimeEvent.ExitConfirmed(runtime.id) else NativeRuntimeEvent.Failed(runtime.id, NativeRuntimeEvent.Failure.Runtime))
            } catch (_: IllegalStateException) {
            }
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size, "exit=$exit")
            assertNull(runtime.surfaceLease)
            assertFalse(runtime.onBack())
            rig.onEvent = {}
            rig.port.onCommand = { _, _ -> }
            assertNotEquals(runtime.id, rig.open().id, "exit=$exit")
        }
    }

    @Test
    fun throwingReadyOrKeepAwakeEnableObserverFailsClosedAndCompletesDriverCleanup() {
        for (observer in listOf("ready", "awake")) {
            val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            rig.port.onCommand = { command, emit ->
                if (command is NativeRuntimeCommand.Stop) emit(NativeRuntimeEvent.Stopped(command.runtime))
            }
            if (observer == "ready") {
                rig.onEvent = { if (it is GameHostEvent.Ready) error("Ready observer failed") }
                rig.interactive(lease)
            } else {
                rig.interactive(lease)
                rig.onAwake = { if (it) error("keepAwake observer failed") }
                rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
                assertEquals(listOf(true, false), rig.awake)
            }
            assertEquals(listOf(GameHostEvent.Failed("native-driver", false)), rig.failures(), observer)
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size, observer)
            assertNull(runtime.surfaceLease)
            assertFalse(runtime.onBack())
            rig.onEvent = {}
            rig.onAwake = {}
            rig.port.onCommand = { _, _ -> }
            rig.open()
        }
    }

    @Test
    fun throwingReadyOrKeepAwakeObserverFencesExactlyItsLeaseBeforeRecoveryCallbacks() {
        for (observer in listOf("ready", "awake")) {
            val rig = Rig(GameLaunch(FIXTURE_ID, preferences, setOf(HostCapability.KeepAwake)))
            val runtime = rig.open()
            val lease = rig.attach(runtime)
            var recovered = false
            rig.onEvent = { event ->
                if (observer == "ready" && event is GameHostEvent.Ready) error("Ready observer failed")
                if (event is GameHostEvent.Failed) {
                    assertEquals(listOf(lease), rig.port.fences, observer)
                    recovered = true
                    runtime.dispose()
                }
            }
            rig.onAwake = { enabled ->
                if (observer == "awake") {
                    if (enabled) error("keepAwake observer failed")
                    assertEquals(listOf(lease), rig.port.fences, "release callback follows the safety fence")
                }
            }
            rig.interactive(lease)
            if (observer == "awake") rig.port.emit(NativeRuntimeEvent.KeepAwake(lease, 1, true))
            assertTrue(recovered, observer)
            assertEquals(listOf(lease), rig.port.fences, observer)
            assertEquals(1, runtime.hostCallbackFailures, observer)
            assertEquals(listOf(GameHostEvent.Failed("native-driver", false)), rig.failures(), observer)
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Detach>().size, observer)
            assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size, observer)
            assertNull(runtime.surfaceLease)
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
        }
    }

    @Test
    fun stoppedFromTheImmediateFenceCannotReleaseAdmissionBeforeStopWasDispatched() {
        val rig = Rig()
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        var fenceObserved = false
        var stopObserved = false
        rig.port.onFence = { fenced ->
            assertEquals(lease, fenced)
            assertTrue(rig.port.sent<NativeRuntimeCommand.Stop>().isEmpty())
            rig.port.emit(NativeRuntimeEvent.Stopped(runtime.id))
            assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen(), "a pre-Stop acknowledgement cannot release the owner")
            fenceObserved = true
        }
        rig.port.onCommand = { command, emit ->
            if (command is NativeRuntimeCommand.Stop) {
                assertTrue(fenceObserved)
                assertIs<NativeRuntimeOpen.Unavailable>(rig.tryOpen())
                emit(NativeRuntimeEvent.Stopped(command.runtime))
                stopObserved = true
            }
        }
        runtime.dispose()
        assertTrue(fenceObserved)
        assertTrue(stopObserved, "premature Stopped must not suppress the real Stop command")
        assertEquals(listOf(lease), rig.port.fences)
        assertEquals(1, rig.port.sent<NativeRuntimeCommand.Stop>().size)
        assertNull(runtime.surfaceLease)
        rig.port.onFence = {}
        rig.port.onCommand = { _, _ -> }
        assertNotEquals(runtime.id, rig.open().id)
    }

    @Test
    fun ownerThreadIsCheckedOnPublicEntriesAndAsynchronousCallbacks() {
        val rig = Rig()
        val runtime = rig.open()
        val lease = rig.attach(runtime)
        rig.onOwnerThread = false
        assertFailsWith<IllegalStateException> { runtime.onBack() }
        assertFailsWith<IllegalStateException> { runtime.suspend() }
        assertFailsWith<IllegalStateException> { runtime.resume() }
        assertFailsWith<IllegalStateException> { runtime.dispose() }
        assertFailsWith<IllegalStateException> { runtime.attach(object : NativeSurface {}, geometry) }
        assertFailsWith<IllegalStateException> { runtime.resize(lease, resizedGeometry) }
        assertFailsWith<IllegalStateException> { runtime.detach(lease) }
        assertFailsWith<IllegalStateException> { runtime.pointer(lease, NativePointer(0, NativePointer.Phase.Down, 0f, 0f)) }
        assertFailsWith<IllegalStateException> { runtime.cancelPointers(lease) }
        assertFailsWith<IllegalStateException> { runtime.leaseFor(object : NativeSurface {}) }
        assertFailsWith<IllegalStateException> { rig.tryOpen() }
        assertFailsWith<IllegalStateException> { rig.port.emit(NativeRuntimeEvent.ContextReady(lease)) }
        rig.onOwnerThread = true
        rig.interactive(lease)
        assertEquals(1, rig.ready().size)
        assertTrue(rig.ownerChecks >= 15)
    }
}
