package com.loveoverflow.tabula.mobile.host

/** Named missing Android mechanisms, never renderer readiness (ADR-0043). */
internal enum class AndroidNativeMissingMechanism(val diagnostic: String) {
    Packaging("native-library-and-verified-assets-not-packaged"),
    Abi("native-abi-and-panic-boundary-not-implemented"),
    Context("embedded-surface-context-not-implemented"),
    Geometry("render-owner-resize-not-implemented"),
    SurfaceRelease("native-window-context-release-not-implemented"),
    Rendering("embedded-render-suspension-not-implemented"),
    Input("bounded-native-input-not-implemented"),
    InputCancel("native-pointer-cancellation-not-implemented"),
    Foreground("native-foreground-command-not-implemented"),
    Back("rust-leave-flow-not-implemented"),
    ServiceReply("native-service-reply-not-implemented"),
    StopJoin("native-worker-stop-join-not-implemented"),
    SurfaceFence("synchronous-render-surface-fence-not-implemented"),
}

/** Typed internal failure with a bounded public diagnostic; carries no game/private state. */
internal class AndroidNativeNotImplemented(val mechanism: AndroidNativeMissingMechanism) :
    IllegalStateException(mechanism.diagnostic)

/**
 * Proposed Android glue boundary for PR #108's existing commands, not a JNI implementation.
 * A future shim must contain panics, retain exact runtime/lease identities and return errors.
 * Callbacks may originate on the renderer; [AndroidNativeRuntimePort] posts them to its owner.
 * No canonical state or per-frame RenderList crosses this boundary (I-5/I-10, ADR-0043).
 */
internal interface AndroidNativeBackend {
    fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit): Result<Unit>
    fun fenceSurface(lease: NativeSurfaceLease): Result<Unit>
}

/**
 * Source skeleton only. It creates no context, match, native window or worker and emits no facts.
 * TODO(phase 6): replace each error only with the named implemented mechanism and its acceptance.
 * The pinned standalone Miniquad start/quit path cannot implement this embedded backend.
 */
internal object UnimplementedAndroidNativeBackend : AndroidNativeBackend {
    override fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit): Result<Unit> =
        Result.failure(AndroidNativeNotImplemented(when (command) {
            // TODO(phase 6): approved ABI/panic containment and selected registry/local runtime.
            is NativeRuntimeCommand.Start -> AndroidNativeMissingMechanism.Abi
            // TODO(phase 6): acquired native window + GPU context on its render owner thread.
            is NativeRuntimeCommand.Attach -> AndroidNativeMissingMechanism.Context
            is NativeRuntimeCommand.Resize -> AndroidNativeMissingMechanism.Geometry
            is NativeRuntimeCommand.Detach -> AndroidNativeMissingMechanism.SurfaceRelease
            is NativeRuntimeCommand.Rendering -> AndroidNativeMissingMechanism.Rendering
            is NativeRuntimeCommand.Pointer -> AndroidNativeMissingMechanism.Input
            is NativeRuntimeCommand.CancelPointers -> AndroidNativeMissingMechanism.InputCancel
            is NativeRuntimeCommand.Foreground -> AndroidNativeMissingMechanism.Foreground
            is NativeRuntimeCommand.Back -> AndroidNativeMissingMechanism.Back
            is NativeRuntimeCommand.ServiceReply -> AndroidNativeMissingMechanism.ServiceReply
            // TODO(phase 6): cancel work, terminate exact worker, release GPU/window, then join.
            is NativeRuntimeCommand.Stop -> AndroidNativeMissingMechanism.StopJoin
        }))

    override fun fenceSurface(lease: NativeSurfaceLease): Result<Unit> =
        // TODO(phase 6): synchronously revoke access to this dispatched lease before returning.
        Result.failure(AndroidNativeNotImplemented(AndroidNativeMissingMechanism.SurfaceFence))
}

/**
 * Adapter skeleton behind NativeRuntimePort, reusing NativeHostSession/NativeRuntimeOwner.
 * Dispatch errors become existing Failed facts. They never become Ready, FramePresented, an
 * exit confirmation or Stopped. In particular, a failed Stop cannot release worker admission.
 * This adapter must not be admitted until packaging and the synchronous surface fence are real.
 */
internal class AndroidNativeRuntimePort(
    private val assertOwnerThread: () -> Unit,
    private val postToOwner: (() -> Unit) -> Unit,
    private val backend: AndroidNativeBackend = UnimplementedAndroidNativeBackend,
) : NativeRuntimePort {
    override fun execute(command: NativeRuntimeCommand, emit: (NativeRuntimeEvent) -> Unit) {
        assertOwnerThread()
        val result = backend.execute(command) { event ->
            postToOwner { assertOwnerThread(); emit(event) }
        }
        if (result.isFailure) {
            val failure = when (command) {
                is NativeRuntimeCommand.Attach, is NativeRuntimeCommand.Resize,
                is NativeRuntimeCommand.Detach, is NativeRuntimeCommand.Rendering -> NativeRuntimeEvent.Failure.GraphicsContext
                is NativeRuntimeCommand.Pointer, is NativeRuntimeCommand.CancelPointers -> NativeRuntimeEvent.Failure.Input
                is NativeRuntimeCommand.Stop -> NativeRuntimeEvent.Failure.Driver
                else -> NativeRuntimeEvent.Failure.Runtime
            }
            emit(NativeRuntimeEvent.Failed(command.runtime, failure))
        }
    }

    override fun fenceSurface(lease: NativeSurfaceLease) {
        assertOwnerThread()
        // Throw the explicit typed error into the existing coordinator's fail-closed path.
        // Returning normally here would falsely claim SurfaceHolder's mandatory safety fence.
        backend.fenceSurface(lease).getOrThrow()
    }
}
