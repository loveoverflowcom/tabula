package com.loveoverflow.tabula.mobile

import com.loveoverflow.tabula.mobile.bridge.GamePreferences
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ReplyCode
import com.loveoverflow.tabula.mobile.bridge.ThemePreference
import com.loveoverflow.tabula.mobile.host.AndroidNativeBackend
import com.loveoverflow.tabula.mobile.host.AndroidNativeMissingMechanism
import com.loveoverflow.tabula.mobile.host.AndroidNativeNotImplemented
import com.loveoverflow.tabula.mobile.host.AndroidNativeRuntimeFactory
import com.loveoverflow.tabula.mobile.host.AndroidNativeRuntimePackaging
import com.loveoverflow.tabula.mobile.host.AndroidNativeRuntimePort
import com.loveoverflow.tabula.mobile.host.GameHostEvent
import com.loveoverflow.tabula.mobile.host.GameLaunch
import com.loveoverflow.tabula.mobile.host.NativeGameRequest
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
import com.loveoverflow.tabula.mobile.host.UnimplementedAndroidNativeBackend
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** Actual Android adapter/gate with controlled thread/callback facts, never GPU/device evidence. */
class AndroidNativeRuntimeSkeletonTest {
    private val launch = GameLaunch(
        "com.example.native-fixture",
        GamePreferences(ThemePreference.Dark, MotionPreference.Reduced, LocalePreference.English),
    )
    private val id = NativeRuntimeId(1)
    private val lease = NativeSurfaceLease(id, 1)
    private val geometry = assertNotNull(NativeSurfaceGeometry.create(640, 480, 2f))

    private fun commands(): List<NativeRuntimeCommand> {
        val request = assertNotNull(NativeGameRequest.admit(launch, setOf(launch.gameId), emptySet()))
        return listOf(
            NativeRuntimeCommand.Start(id, request, true),
            NativeRuntimeCommand.Attach(lease, object : NativeSurface {}, geometry),
            NativeRuntimeCommand.Resize(lease, geometry),
            NativeRuntimeCommand.Detach(lease),
            NativeRuntimeCommand.Rendering(lease, false),
            NativeRuntimeCommand.Pointer(lease, NativePointer(0, NativePointer.Phase.Down, 1f, 2f)),
            NativeRuntimeCommand.CancelPointers(lease),
            NativeRuntimeCommand.Foreground(id, false),
            NativeRuntimeCommand.Back(id),
            NativeRuntimeCommand.ServiceReply(id, 1, ReplyCode.Denied),
            NativeRuntimeCommand.Stop(id),
        )
    }

    @Test
    fun packagingIsTypedUnavailableAndCannotAdvertiseAnyRuntimeOrService() {
        val error = assertIs<AndroidNativeNotImplemented>(AndroidNativeRuntimePackaging.verify().exceptionOrNull())
        assertEquals(AndroidNativeMissingMechanism.Packaging, error.mechanism)
        assertTrue(AndroidNativeRuntimePackaging.packagedGameIds.isEmpty())
        assertTrue(AndroidNativeRuntimePackaging.supportedCapabilities.isEmpty())
    }

    @Test
    fun repeatedFactoryAdmissionNeverReservesAWorkerReadsAClockOrDispatchesStart() {
        val commands = mutableListOf<NativeRuntimeCommand>()
        val events = mutableListOf<GameHostEvent>()
        val awake = mutableListOf<Boolean>()
        var clockReads = 0
        val port = object : NativeRuntimePort {
            override fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit) { commands += command }
            override fun fenceSurface(lease: NativeSurfaceLease) { error("preflight must not reach a surface") }
        }
        val factory = AndroidNativeRuntimeFactory({}, { clockReads++; 1 }, port)
        repeat(8) {
            val result = assertIs<NativeRuntimeOpen.Unavailable>(factory.open(launch, it % 2 == 0, events::add, awake::add))
            assertEquals(AndroidNativeMissingMechanism.Packaging.diagnostic, result.reason)
        }
        assertEquals(0, clockReads)
        assertTrue(commands.isEmpty())
        assertTrue(events.isEmpty())
        assertTrue(awake.isEmpty())
    }

    @Test
    fun everyExistingCommandNamesItsMissingMechanismWithoutPublishingBackendFacts() {
        val expected = listOf(
            AndroidNativeMissingMechanism.Abi, AndroidNativeMissingMechanism.Context,
            AndroidNativeMissingMechanism.Geometry, AndroidNativeMissingMechanism.SurfaceRelease,
            AndroidNativeMissingMechanism.Rendering, AndroidNativeMissingMechanism.Input,
            AndroidNativeMissingMechanism.InputCancel, AndroidNativeMissingMechanism.Foreground,
            AndroidNativeMissingMechanism.Back, AndroidNativeMissingMechanism.ServiceReply,
            AndroidNativeMissingMechanism.StopJoin,
        )
        val events = mutableListOf<NativeRuntimeEvent>()
        assertEquals(11, commands().size)
        commands().zip(expected).forEach { (command, mechanism) ->
            val failure = assertIs<AndroidNativeNotImplemented>(UnimplementedAndroidNativeBackend.execute(command, events::add).exceptionOrNull())
            assertEquals(mechanism, failure.mechanism)
            assertTrue(failure.message.orEmpty().length in 1..80)
        }
        assertTrue(events.isEmpty(), "no context/resources/input/frame/exit/stop receipt exists")
    }

    @Test
    fun portMapsAllCommandErrorsToFailuresWithoutReadinessExitOrStopSuccess() {
        val events = mutableListOf<NativeRuntimeEvent>()
        val port = AndroidNativeRuntimePort({}, { it() })
        commands().forEach { port.execute(it, events::add) }
        assertEquals(11, events.size)
        assertTrue(events.all { it is NativeRuntimeEvent.Failed && it.runtime == id })
        assertEquals(NativeRuntimeEvent.Failure.Driver, assertIs<NativeRuntimeEvent.Failed>(events.last()).code)
    }

    @Test
    fun aMissingFenceCannotReturnAFalseSuccessfulFence() {
        val port = AndroidNativeRuntimePort({}, { it() })
        repeat(2) {
            val error = assertFailsWith<AndroidNativeNotImplemented> { port.fenceSurface(lease) }
            assertEquals(AndroidNativeMissingMechanism.SurfaceFence, error.mechanism)
        }
    }

    @Test
    fun forcedTestInventoryCannotTurnFailedStartOrStopIntoOwnerRelease() {
        val owner = NativeRuntimeOwner(setOf(launch.gameId), emptySet()) {}
        val port = AndroidNativeRuntimePort({}, { it() })
        val events = mutableListOf<GameHostEvent>()
        val first = assertIs<NativeRuntimeOpen.Opened>(owner.open(launch, port, true, { 1 }, events::add, {}))
        first.runtime.dispose()
        val retry = assertIs<NativeRuntimeOpen.Unavailable>(owner.open(launch, port, true, { 2 }, events::add, {}))
        assertEquals("native-worker-stopping-or-active", retry.reason)
        assertEquals(1, events.size)
        assertIs<GameHostEvent.Failed>(events.single())
    }

    @Test
    fun backendCallbacksArePostedBeforeTheyReachTheExistingCoordinatorOwner() {
        val posts = mutableListOf<() -> Unit>()
        val events = mutableListOf<NativeRuntimeEvent>()
        var ownerThread = true
        var callback: ((NativeRuntimeEvent) -> Unit)? = null
        val backend = object : AndroidNativeBackend {
            override fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit): Result<Unit> {
                callback = emit
                return Result.success(Unit) // Named callback test double, not the production stub.
            }
            override fun fenceSurface(lease: NativeSurfaceLease): Result<Unit> = error("unused test operation")
        }
        val port = AndroidNativeRuntimePort({ check(ownerThread) }, { posts += it }, backend)
        port.execute(NativeRuntimeCommand.Back(id), events::add)
        ownerThread = false
        assertNotNull(callback)(NativeRuntimeEvent.Failed(id, NativeRuntimeEvent.Failure.Runtime))
        assertTrue(events.isEmpty())
        assertEquals(1, posts.size)
        ownerThread = true
        posts.single()()
        assertEquals(listOf<NativeRuntimeEvent>(NativeRuntimeEvent.Failed(id, NativeRuntimeEvent.Failure.Runtime)), events)
    }

    @Test
    fun adapterRejectsWrongOwnerBeforeDispatchOrFence() {
        val port = AndroidNativeRuntimePort({ error("wrong-owner") }, { it() })
        assertFailsWith<IllegalStateException> { port.execute(NativeRuntimeCommand.Back(id)) {} }
        assertFailsWith<IllegalStateException> { port.fenceSurface(lease) }
    }
}
