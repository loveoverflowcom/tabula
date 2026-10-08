package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.HostCapability
import com.loveoverflow.tabula.mobile.bridge.LocalePreference
import com.loveoverflow.tabula.mobile.bridge.MotionPreference
import com.loveoverflow.tabula.mobile.bridge.ReplyCode
import com.loveoverflow.tabula.mobile.bridge.ThemePreference

/** Process-local worker identity, distinct from a surface revision (ADR-0043). */
@ConsistentCopyVisibility
data class NativeRuntimeId internal constructor(val value: Long)

/** A callback ticket for one worker and one usable surface configuration (ADR-0043). */
@ConsistentCopyVisibility
data class NativeSurfaceLease internal constructor(val runtime: NativeRuntimeId, val revision: Long)

/** Platform-owned surface reference. This carries no match, projection or drawing data (I-5/I-10). */
interface NativeSurface

/**
 * Validated geometry of the game view, in physical pixels. The outer CMP shell consumes safe
 * insets; adapters must not apply them again. Rust converts local input/geometry using [density]
 * exactly once. Zero-size/background layouts are not attachable (ADR-0043).
 */
class NativeSurfaceGeometry private constructor(val widthPx: Int, val heightPx: Int, val density: Float) {
    override fun equals(other: Any?): Boolean = other is NativeSurfaceGeometry &&
        widthPx == other.widthPx && heightPx == other.heightPx && density == other.density

    override fun hashCode(): Int = 31 * (31 * widthPx + heightPx) + density.hashCode()

    companion object {
        /** Rejects hostile/unsupported dimensions without starting or resizing a runtime. */
        fun create(widthPx: Int, heightPx: Int, density: Float): NativeSurfaceGeometry? =
            if (widthPx in 1..16_384 && heightPx in 1..16_384 && density.isFinite() && density in 0.5f..16f) {
                NativeSurfaceGeometry(widthPx, heightPx, density)
            } else null
    }
}

/** Native input facts in game-view physical pixels, never a game command (I-10). */
data class NativePointer(val id: Int, val phase: Phase, val xPx: Float, val yPx: Float) {
    enum class Phase { Down, Move, Up }

    internal fun isBounded(): Boolean = id in 0..31 && xPx.isFinite() && yPx.isFinite() &&
        xPx in -32_768f..32_768f && yPx in -32_768f..32_768f
}

/**
 * Admitted immutable launch facts. A public discovery entry/web manifest is not a packaged native
 * runtime. The backend still resolves the opaque id through the Rust registry (I-9/ADR-0043).
 */
class NativeGameRequest private constructor(
    val gameId: String,
    val preferences: NativeGamePreferences,
    private val services: Set<HostCapability>,
) {
    /** A defensive snapshot; callers cannot expand this launch's host authority. */
    val capabilities: Set<HostCapability> get() = services.toSet()
    internal fun grants(capability: HostCapability): Boolean = capability in services

    companion object {
        internal val idPattern = Regex("^[a-z0-9]+(\\.[a-z0-9-]+)+$")

        internal fun admit(launch: GameLaunch, packagedIds: Set<String>, supported: Set<HostCapability>): NativeGameRequest? {
            if (launch.gameId.length > 80 || !idPattern.matches(launch.gameId) || launch.gameId !in packagedIds) return null
            val preferences = launch.preferences ?: return null
            if (preferences.theme == ThemePreference.System) return null
            val services = launch.capabilities.toSet()
            if (!supported.containsAll(services)) return null
            return NativeGameRequest(
                launch.gameId,
                NativeGamePreferences(preferences.theme, preferences.motion == MotionPreference.Reduced, preferences.locale),
                services,
            )
        }
    }
}

/**
 * Explicit launch-only device preferences. The existing shell resolves motion to Reduced or its
 * normal/System value; the native backend receives that result as a boolean, never polls device
 * settings. [theme] is admitted only when explicit, and locale is the existing vi/en enum.
 */
class NativeGamePreferences internal constructor(
    val theme: ThemePreference,
    val reducedMotion: Boolean,
    val locale: LocalePreference,
)

/**
 * Thin control operations for an eventual Rust/Macroquad backend, not per-frame draw commands.
 * All calls and callbacks are serialized on the platform owner thread. The backend owns its GPU
 * context/render thread and schedules frames itself; it must never run a second match on resize.
 */
sealed interface NativeRuntimeCommand {
    val runtime: NativeRuntimeId

    /** CPU/runtime initialization only; GPU work needs a current attached surface/context. */
    data class Start(override val runtime: NativeRuntimeId, val request: NativeGameRequest, val foreground: Boolean) : NativeRuntimeCommand
    data class Attach(val lease: NativeSurfaceLease, val surface: NativeSurface, val geometry: NativeSurfaceGeometry) : NativeRuntimeCommand {
        override val runtime get() = lease.runtime
    }
    /** Retires old tickets on geometry/DPI or foreground changes, retaining runtime/match. */
    data class Resize(val lease: NativeSurfaceLease, val geometry: NativeSurfaceGeometry) : NativeRuntimeCommand {
        override val runtime get() = lease.runtime
    }
    /** Remaining cleanup after the immediate fenceSurface operation; not the synchronous fence. */
    data class Detach(val lease: NativeSurfaceLease) : NativeRuntimeCommand { override val runtime get() = lease.runtime }
    data class Rendering(val lease: NativeSurfaceLease, val enabled: Boolean) : NativeRuntimeCommand { override val runtime get() = lease.runtime }
    data class Pointer(val lease: NativeSurfaceLease, val input: NativePointer) : NativeRuntimeCommand { override val runtime get() = lease.runtime }
    data class CancelPointers(val lease: NativeSurfaceLease) : NativeRuntimeCommand { override val runtime get() = lease.runtime }
    /** Rendering/input suspension is not an authority/timer pause (doc 00 §6.3). */
    data class Foreground(override val runtime: NativeRuntimeId, val active: Boolean) : NativeRuntimeCommand
    data class Back(override val runtime: NativeRuntimeId) : NativeRuntimeCommand
    data class ServiceReply(override val runtime: NativeRuntimeId, val requestId: Int, val code: ReplyCode) : NativeRuntimeCommand

    /**
     * Cancel pending CPU/GPU work, stop frames, release surfaces/resources and join the renderer
     * without blocking the UI thread. Only then emit Stopped. Returning from execute is not a join.
     */
    data class Stop(override val runtime: NativeRuntimeId) : NativeRuntimeCommand
}

/** Backend facts. Adapters marshal them from the render thread to the owner thread (ADR-0043). */
sealed interface NativeRuntimeEvent {
    val runtime: NativeRuntimeId
    data class ContextReady(val lease: NativeSurfaceLease) : NativeRuntimeEvent { override val runtime get() = lease.runtime }
    /** Required verified packs/decoded resources have been uploaded on the valid context thread. */
    data class ResourcesReady(val lease: NativeSurfaceLease) : NativeRuntimeEvent { override val runtime get() = lease.runtime }
    data class InputReady(val lease: NativeSurfaceLease) : NativeRuntimeEvent { override val runtime get() = lease.runtime }
    /**
     * A successfully flushed interactive gameplay frame, not end_frame's queued draws/loading UI.
     * Numbers strictly increase within a lease. A frame before the readiness facts is insufficient.
     */
    data class FramePresented(val lease: NativeSurfaceLease, val number: Long) : NativeRuntimeEvent { override val runtime get() = lease.runtime }
    data class KeepAwake(val lease: NativeSurfaceLease, val requestId: Int, val enabled: Boolean) : NativeRuntimeEvent { override val runtime get() = lease.runtime }
    data class Failed(override val runtime: NativeRuntimeId, val code: Failure) : NativeRuntimeEvent
    data class ExitConfirmed(override val runtime: NativeRuntimeId) : NativeRuntimeEvent
    /** The exact retiring worker is joined and its native/GPU references are released. */
    data class Stopped(override val runtime: NativeRuntimeId) : NativeRuntimeEvent

    enum class Failure { Runtime, GraphicsContext, Resources, Input, Driver }
}

/**
 * Adapter port only. Android has a fail-closed source skeleton, not a shipping native backend.
 * Pinned Miniquad start/quit alone cannot satisfy this contract. No credentials, audio/voice,
 * state, RenderList or executable
 * downloads cross it. An FFI shim must contain panics and needs its own reviewed policy boundary.
 */
interface NativeRuntimePort {
    fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit)

    /**
     * Immediate, idempotent old-surface access/input fence, safe even when a host callback reenters
     * during execute. It must return only after the worker cannot touch that surface again. The
     * queued Detach command handles remaining cleanup; this is not full runtime Stop/join.
     * The lease is the last dispatched Attach/Resize ticket for that surface; pending revisions
     * are retired by the coordinator without asking the backend to recognize an unseen ticket.
     * Android SurfaceHolder cannot wait for the owner's deferred effect queue to establish this.
     * Worker callbacks must be posted without synchronously waiting for the UI callback to finish,
     * otherwise an owner-thread surface fence could deadlock against the worker's callback wait.
     */
    fun fenceSurface(lease: NativeSurfaceLease)
}
