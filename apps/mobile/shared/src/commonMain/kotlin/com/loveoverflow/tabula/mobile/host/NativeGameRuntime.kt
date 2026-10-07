package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.HostCapability

/** Result of bounded native admission. Unavailable never starts a backend or reports Ready. */
sealed interface NativeRuntimeOpen {
    data class Opened(val runtime: NativeGameRuntime) : NativeRuntimeOpen
    data class Unavailable(val reason: String) : NativeRuntimeOpen
}

/**
 * One process-wide admission gate per Macroquad backend, retained across screen entries.
 * Packaged ids and supported services are explicit adapter inventory, not the public catalog.
 * Production supplies no such backend/inventory today (ADR-0043). [assertOwnerThread] must check
 * the platform UI/owner thread on every entry, including callbacks. Never replace this owner to
 * bypass a worker whose asynchronous Stop has not acknowledged termination.
 */
class NativeRuntimeOwner(
    packagedGameIds: Set<String>,
    supportedCapabilities: Set<HostCapability>,
    private val assertOwnerThread: () -> Unit,
) {
    private val packagedIds = packagedGameIds.toSet()
    private val supported = supportedCapabilities.toSet()
    private var nextId = 0L
    private var active: NativeRuntimeId? = null

    /**
     * Called once on game-screen entry, never per recomposition. Preferences must already be
     * resolved. Start receives the actual initial foreground state, including a background entry.
     * A port is an injected prototype/test adapter until native packaging/device gates are met.
     */
    fun open(
        launch: GameLaunch,
        port: NativeRuntimePort,
        initiallyForeground: Boolean,
        monotonicMs: () -> Long,
        onEvent: (GameHostEvent) -> Unit,
        onKeepAwake: (Boolean) -> Unit,
    ): NativeRuntimeOpen {
        assertOwnerThread()
        val request = NativeGameRequest.admit(launch, packagedIds, supported)
            ?: return NativeRuntimeOpen.Unavailable("native-launch-unavailable")
        if (active != null) return NativeRuntimeOpen.Unavailable("native-worker-stopping-or-active")
        if (nextId == Long.MAX_VALUE) return NativeRuntimeOpen.Unavailable("native-worker-id-exhausted")
        val startedAtMs = monotonicMs().coerceAtLeast(0)
        nextId += 1
        val id = NativeRuntimeId(nextId)
        // Reserve before Start: a port may synchronously invoke its callback.
        active = id
        val runtime = NativeGameRuntime(
            NativeHostSession(id, request, startedAtMs, initiallyForeground), port, assertOwnerThread,
            monotonicMs, onEvent, onKeepAwake,
        ) { if (active == id) active = null }
        runtime.start()
        return NativeRuntimeOpen.Opened(runtime)
    }
}

/**
 * Effect executor for NativeHostSession and GameRuntimeControls (ADR-0043). It neither owns rules
 * nor schedules/draws frames. The serial queue tolerates synchronous/reentrant adapter callbacks.
 * Disposal initiates stop; only a truthful Stopped callback releases the owner's admission gate.
 */
class NativeGameRuntime internal constructor(
    private val session: NativeHostSession,
    private val port: NativeRuntimePort,
    private val assertOwnerThread: () -> Unit,
    private val monotonicMs: () -> Long,
    private val onEvent: (GameHostEvent) -> Unit,
    private val onKeepAwake: (Boolean) -> Unit,
    private val releaseOwner: () -> Unit,
) : GameRuntimeControls {
    private val effects = ArrayDeque<NativeHostEffect>()
    private var draining = false
    private var compositionDisposed = false
    /** Diagnostic count; a throwing shell/service callback cannot strand native cleanup. */
    var hostCallbackFailures: Int = 0
        private set
    val id: NativeRuntimeId get() = session.id
    val surfaceLease: NativeSurfaceLease? get() = session.lease

    /** Current revision only if this is still the attached platform surface. */
    fun leaseFor(surface: NativeSurface): NativeSurfaceLease? {
        assertOwnerThread()
        return session.leaseFor(surface)
    }

    internal fun start() = run(session.start())

    /** Returns the fresh ticket after retiring the previous surface; this does not start a match. */
    fun attach(surface: NativeSurface, geometry: NativeSurfaceGeometry): NativeSurfaceLease? {
        assertOwnerThread()
        run(session.attach(surface, geometry))
        return session.lease
    }

    /** Retires old callbacks on meaningful geometry/DPI changes; unchanged geometry is a no-op. */
    fun resize(lease: NativeSurfaceLease, geometry: NativeSurfaceGeometry): NativeSurfaceLease? {
        assertOwnerThread()
        run(session.resize(lease, geometry))
        return session.lease
    }

    fun detach(lease: NativeSurfaceLease) { assertOwnerThread(); run(session.detach(lease)) }
    fun pointer(lease: NativeSurfaceLease, input: NativePointer) { assertOwnerThread(); run(session.pointer(lease, input)) }
    fun cancelPointers(lease: NativeSurfaceLease) { assertOwnerThread(); run(session.cancelPointers(lease)) }

    override fun onBack(): Boolean {
        assertOwnerThread()
        val outcome = session.back()
        run(outcome)
        return outcome.isNotEmpty()
    }

    override fun suspend() { assertOwnerThread(); run(session.foreground(false)) }
    override fun resume() { assertOwnerThread(); run(session.foreground(true)) }
    override fun dispose() {
        assertOwnerThread()
        if (compositionDisposed) return
        compositionDisposed = true
        run(session.stop())
    }

    private fun event(value: NativeRuntimeEvent) {
        assertOwnerThread()
        run(session.event(value, monotonicMs().coerceAtLeast(0)))
    }

    private fun run(values: List<NativeHostEffect>) {
        if (draining) {
            enqueue(values)
            return
        }
        draining = true
        try {
            enqueue(values)
            while (effects.isNotEmpty()) {
                when (val effect = effects.removeFirst()) {
                    is NativeHostEffect.Command -> try {
                        if (session.permits(effect.value)) {
                            session.commandStarted(effect.value)
                            port.execute(effect.value, ::event)
                        }
                    } catch (_: Exception) {
                        // A failed Stop is not proof of termination: admission remains closed.
                        enqueue(session.fail(NativeRuntimeEvent.Failure.Driver))
                    }
                    is NativeHostEffect.Notify -> if (!compositionDisposed && session.acceptNotification(effect)) hostCallback { onEvent(effect.value) }
                    is NativeHostEffect.KeepAwake -> if ((!compositionDisposed || !effect.enabled) && session.permitsService(effect)) hostCallback { onKeepAwake(effect.enabled) }
                    NativeHostEffect.ReleaseOwner -> releaseOwner()
                }
            }
        } finally {
            draining = false
        }
    }

    private fun enqueue(values: List<NativeHostEffect>) {
        // SurfaceHolder destruction can reenter from a host callback while this queue drains.
        // Its safety fence must complete now, not after the outer execute/onEvent returns.
        val fenceFailures = mutableListOf<NativeHostEffect>()
        for (effect in values) {
            val detach = (effect as? NativeHostEffect.Command)?.value as? NativeRuntimeCommand.Detach ?: continue
            try {
                port.fenceSurface(detach.lease)
            } catch (_: Exception) {
                // A throwing fence is not a successful fence. Retire the worker and fail closed;
                // such a backend cannot be admitted as a production Android adapter.
                fenceFailures.addAll(session.fail(NativeRuntimeEvent.Failure.GraphicsContext))
            }
        }
        effects.addAll(values)
        if (fenceFailures.isNotEmpty()) enqueue(fenceFailures)
    }

    private fun hostCallback(action: () -> Unit) {
        try {
            action()
        } catch (_: Exception) {
            if (hostCallbackFailures < Int.MAX_VALUE) hostCallbackFailures += 1
            enqueue(session.fail(NativeRuntimeEvent.Failure.Driver))
        }
    }
}
