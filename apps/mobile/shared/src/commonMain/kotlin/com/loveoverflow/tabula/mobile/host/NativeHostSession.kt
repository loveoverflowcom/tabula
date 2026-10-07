package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.ReplyCode

/** Pure lifecycle decisions, kept separate from ADR-0033's historical document GameSession. */
internal sealed interface NativeHostEffect {
    data class Command(val value: NativeRuntimeCommand) : NativeHostEffect
    data class Notify(val value: GameHostEvent, val lease: NativeSurfaceLease? = null) : NativeHostEffect
    data class KeepAwake(val enabled: Boolean, val lease: NativeSurfaceLease? = null) : NativeHostEffect
    data object ReleaseOwner : NativeHostEffect
}

/**
 * One native worker/local match and successive surface leases (ADR-0043, I-10). It performs no
 * platform I/O. All mutations are confined to one owner thread by NativeGameRuntime.
 */
internal class NativeHostSession(
    val id: NativeRuntimeId,
    private val request: NativeGameRequest,
    private val startedAtMs: Long,
    foreground: Boolean,
) {
    enum class Phase { Starting, Interactive, Stopping, Stopped }
    var phase = Phase.Starting
        private set
    var lease: NativeSurfaceLease? = null
        private set
    private var geometry: NativeSurfaceGeometry? = null
    private var surface: NativeSurface? = null
    // Planned revisions can change while effects wait. Only dispatched Attach/Resize tickets
    // identify a surface the port actually knows, including for the synchronous destruction fence.
    private var dispatchedSurfaceLease: NativeSurfaceLease? = null
    private var surfaceRevision = 0L
    private var foreground = foreground
    private var contextReady = false
    private var resourcesReady = false
    private var inputReady = false
    private var lastFrame = 0L
    private var notifiedReady = false
    private var queuedReady: NativeSurfaceLease? = null
    private var keepAwake = false
    private var lastServiceId = 0
    private var stopSent = false
    private val pointers = mutableSetOf<Int>()

    fun start(): List<NativeHostEffect> = command(NativeRuntimeCommand.Start(id, request, foreground))
    fun leaseFor(value: NativeSurface): NativeSurfaceLease? = if (surface === value) lease else null

    /** A synchronous failure/disposal can retire commands already waiting in the executor queue. */
    fun permits(value: NativeRuntimeCommand): Boolean = value.runtime == id && when (value) {
        is NativeRuntimeCommand.Start -> live()
        is NativeRuntimeCommand.Attach -> current(value.lease)
        is NativeRuntimeCommand.Resize -> current(value.lease)
        is NativeRuntimeCommand.Rendering -> !value.enabled || (current(value.lease) && foreground)
        is NativeRuntimeCommand.Pointer -> current(value.lease) && phase == Phase.Interactive && foreground
        is NativeRuntimeCommand.Foreground -> live() && value.active == foreground
        is NativeRuntimeCommand.Back -> phase == Phase.Interactive && foreground
        is NativeRuntimeCommand.ServiceReply -> live()
        is NativeRuntimeCommand.Stop -> phase == Phase.Stopping
        is NativeRuntimeCommand.CancelPointers, is NativeRuntimeCommand.Detach -> true
    }

    fun commandStarted(value: NativeRuntimeCommand) {
        when (value) {
            is NativeRuntimeCommand.Attach -> dispatchedSurfaceLease = value.lease
            is NativeRuntimeCommand.Resize -> dispatchedSurfaceLease = value.lease
            is NativeRuntimeCommand.Stop -> stopSent = true
            else -> Unit
        }
    }

    fun acceptNotification(value: NativeHostEffect.Notify): Boolean {
        if (value.value !is GameHostEvent.Ready) return true
        if (queuedReady == value.lease) queuedReady = null
        val fresh = !notifiedReady && value.lease != null && current(value.lease) && phase == Phase.Interactive && foreground
        if (fresh) notifiedReady = true
        return fresh
    }

    fun permitsService(value: NativeHostEffect.KeepAwake): Boolean = !value.enabled ||
        (value.lease != null && current(value.lease) && phase == Phase.Interactive && foreground)

    fun attach(surface: NativeSurface, value: NativeSurfaceGeometry): List<NativeHostEffect> {
        if (!live()) return emptyList()
        val retire = detach()
        val ticket = nextLease() ?: return retire + fail(NativeRuntimeEvent.Failure.Driver)
        lease = ticket
        geometry = value
        this.surface = surface
        return retire + command(NativeRuntimeCommand.Attach(ticket, surface, value), NativeRuntimeCommand.Rendering(ticket, foreground))
    }

    fun resize(expected: NativeSurfaceLease, value: NativeSurfaceGeometry): List<NativeHostEffect> {
        if (!current(expected) || value == geometry) return emptyList()
        val currentSurface = surface ?: return emptyList()
        val ticket = nextLease() ?: return fail(NativeRuntimeEvent.Failure.Driver)
        resetReadiness()
        lease = ticket
        geometry = value
        return releaseAwake() + disableDispatchedSurface() + command(
            configureSurface(ticket, currentSurface, value), NativeRuntimeCommand.Rendering(ticket, foreground),
        )
    }

    fun detach(expected: NativeSurfaceLease? = lease): List<NativeHostEffect> {
        if (expected == null || !current(expected)) return emptyList()
        val dispatched = dispatchedSurfaceLease
        dispatchedSurfaceLease = null
        lease = null
        geometry = null
        surface = null
        resetReadiness()
        // A queued Attach that never reached the port needs no native cleanup. A pending Resize
        // must fence the last dispatched ticket, rather than a revision unknown to the worker.
        return releaseAwake() + if (dispatched != null) command(
            NativeRuntimeCommand.CancelPointers(dispatched), NativeRuntimeCommand.Rendering(dispatched, false), NativeRuntimeCommand.Detach(dispatched),
        ) else emptyList()
    }

    fun foreground(active: Boolean): List<NativeHostEffect> {
        if (!live() || foreground == active) return emptyList()
        foreground = active
        val previous = lease
        val value = geometry
        val currentSurface = surface
        resetReadiness()
        val cancel = disableDispatchedSurface()
        // Both boundaries retire frame/resource/input callbacks, including delayed pre-pause frames.
        val configure = if (previous != null && value != null && currentSurface != null) {
            val ticket = nextLease() ?: return cancel + fail(NativeRuntimeEvent.Failure.Driver)
            lease = ticket
            command(configureSurface(ticket, currentSurface, value), NativeRuntimeCommand.Rendering(ticket, active))
        } else emptyList()
        return cancel + releaseAwake() + command(NativeRuntimeCommand.Foreground(id, active)) + configure
    }

    fun pointer(expected: NativeSurfaceLease, value: NativePointer): List<NativeHostEffect> {
        if (!current(expected) || phase != Phase.Interactive || !foreground) return emptyList()
        if (!value.isBounded()) return cancelPointers(expected)
        when (value.phase) {
            NativePointer.Phase.Down -> {
                if (value.id in pointers || pointers.size >= 10) return cancelPointers(expected)
                pointers += value.id
            }
            NativePointer.Phase.Move -> if (value.id !in pointers) return emptyList()
            NativePointer.Phase.Up -> if (!pointers.remove(value.id)) return emptyList()
        }
        return command(NativeRuntimeCommand.Pointer(expected, value))
    }

    fun cancelPointers(expected: NativeSurfaceLease): List<NativeHostEffect> {
        if (!current(expected)) return emptyList()
        pointers.clear()
        return command(NativeRuntimeCommand.CancelPointers(expected))
    }

    fun back(): List<NativeHostEffect> =
        if (phase == Phase.Interactive && foreground && lease != null) command(NativeRuntimeCommand.Back(id)) else emptyList()

    fun event(value: NativeRuntimeEvent, nowMs: Long): List<NativeHostEffect> {
        if (value.runtime != id) return emptyList()
        if (value is NativeRuntimeEvent.Stopped) {
            if (phase != Phase.Stopping || !stopSent) return emptyList()
            phase = Phase.Stopped
            return listOf(NativeHostEffect.ReleaseOwner)
        }
        if (!live()) return emptyList()
        return when (value) {
            is NativeRuntimeEvent.ContextReady -> { if (current(value.lease)) contextReady = true; emptyList() }
            is NativeRuntimeEvent.ResourcesReady -> { if (current(value.lease)) resourcesReady = true; emptyList() }
            is NativeRuntimeEvent.InputReady -> { if (current(value.lease) && foreground) inputReady = true; emptyList() }
            is NativeRuntimeEvent.FramePresented -> frame(value, nowMs)
            is NativeRuntimeEvent.KeepAwake -> service(value)
            is NativeRuntimeEvent.Failed -> fail(value.code)
            is NativeRuntimeEvent.ExitConfirmed -> stop() + NativeHostEffect.Notify(GameHostEvent.Exited)
            is NativeRuntimeEvent.Stopped -> emptyList()
        }
    }

    fun fail(code: NativeRuntimeEvent.Failure): List<NativeHostEffect> {
        if (!live()) return emptyList()
        return stop() + NativeHostEffect.Notify(GameHostEvent.Failed("native-${code.name.lowercase()}", shownByGame = false))
    }

    fun stop(): List<NativeHostEffect> {
        if (!live()) return emptyList()
        val retire = detach()
        // Retire ordinary callbacks before any effect can reenter the coordinator.
        phase = Phase.Stopping
        return retire + releaseAwake() + command(NativeRuntimeCommand.Stop(id))
    }

    private fun frame(value: NativeRuntimeEvent.FramePresented, nowMs: Long): List<NativeHostEffect> {
        if (!current(value.lease) || value.number <= lastFrame || value.number <= 0) return emptyList()
        lastFrame = value.number // Early/background frames cannot be replayed after prerequisites arrive.
        if (!foreground || !contextReady || !resourcesReady || !inputReady) return emptyList()
        phase = Phase.Interactive
        if (notifiedReady || queuedReady == value.lease) return emptyList()
        queuedReady = value.lease
        val bootMs = if (nowMs <= startedAtMs) 0 else (nowMs - startedAtMs).coerceAtMost(3_600_000).toInt()
        return listOf(NativeHostEffect.Notify(GameHostEvent.Ready(bootMs), value.lease))
    }

    private fun service(value: NativeRuntimeEvent.KeepAwake): List<NativeHostEffect> {
        if (!current(value.lease) || value.requestId <= lastServiceId || value.requestId <= 0) return emptyList()
        lastServiceId = value.requestId
        val code = when {
            !request.grants(HostCapability.KeepAwake) -> ReplyCode.Denied
            phase != Phase.Interactive || !foreground -> ReplyCode.Unsupported
            else -> ReplyCode.Ok
        }
        val effects = if (code == ReplyCode.Ok) {
            keepAwake = value.enabled
            listOf(NativeHostEffect.KeepAwake(value.enabled, value.lease))
        } else emptyList()
        return effects + command(NativeRuntimeCommand.ServiceReply(id, value.requestId, code))
    }

    private fun releaseAwake(): List<NativeHostEffect> {
        if (!keepAwake) return emptyList()
        keepAwake = false
        return listOf(NativeHostEffect.KeepAwake(false))
    }

    private fun resetReadiness() {
        phase = Phase.Starting
        contextReady = false
        resourcesReady = false
        inputReady = false
        lastFrame = 0
        pointers.clear()
    }

    private fun nextLease(): NativeSurfaceLease? {
        if (surfaceRevision == Long.MAX_VALUE) return null
        surfaceRevision += 1
        return NativeSurfaceLease(id, surfaceRevision)
    }

    private fun configureSurface(
        ticket: NativeSurfaceLease, value: NativeSurface, geometry: NativeSurfaceGeometry,
    ): NativeRuntimeCommand = if (dispatchedSurfaceLease == null) {
        NativeRuntimeCommand.Attach(ticket, value, geometry)
    } else NativeRuntimeCommand.Resize(ticket, geometry)

    private fun disableDispatchedSurface(): List<NativeHostEffect> = dispatchedSurfaceLease?.let {
        command(NativeRuntimeCommand.CancelPointers(it), NativeRuntimeCommand.Rendering(it, false))
    } ?: emptyList()

    private fun live() = phase == Phase.Starting || phase == Phase.Interactive
    private fun current(ticket: NativeSurfaceLease) = live() && lease == ticket
    private fun command(vararg values: NativeRuntimeCommand) = values.map { NativeHostEffect.Command(it) }
}
