package com.loveoverflow.tabula.mobile.host

import com.loveoverflow.tabula.mobile.bridge.HostCapability

/**
 * Packaging/admission seam, independent of the public discovery catalog (I-9, ADR-0043/0045).
 * TODO(phase 6): derive these facts from selected registry entries, actual ABI library identity,
 * compatible contract version and integrity-checked game/font/license assets in the built APK.
 * A manifest, source feature, or native file's mere presence cannot advertise an admitted game.
 */
internal object AndroidNativeRuntimePackaging {
    val packagedGameIds: Set<String> get() = emptySet()
    val supportedCapabilities: Set<HostCapability> get() = emptySet()

    /** No Android runtime artifact is produced by this skeleton; even a catalog id stays closed. */
    fun verify(): Result<Unit> =
        Result.failure(AndroidNativeNotImplemented(AndroidNativeMissingMechanism.Packaging))
}

/**
 * One retained owner, using the merged PR #108 admission/lifecycle implementation unchanged.
 * Preflight fails before reserving an owner or dispatching Start, so the skeleton cannot create
 * a nonexistent worker and then fabricate a join receipt to release it (ADR-0043).
 */
internal class AndroidNativeRuntimeFactory(
    private val assertOwnerThread: () -> Unit,
    private val monotonicMs: () -> Long,
    private val port: NativeRuntimePort,
) {
    private val owner = NativeRuntimeOwner(
        AndroidNativeRuntimePackaging.packagedGameIds,
        AndroidNativeRuntimePackaging.supportedCapabilities,
        assertOwnerThread,
    )

    fun open(
        launch: GameLaunch,
        initiallyForeground: Boolean,
        onEvent: (GameHostEvent) -> Unit,
        onKeepAwake: (Boolean) -> Unit,
    ): NativeRuntimeOpen {
        assertOwnerThread()
        val unavailable = AndroidNativeRuntimePackaging.verify().exceptionOrNull()
        if (unavailable != null) return NativeRuntimeOpen.Unavailable(
            (unavailable as? AndroidNativeNotImplemented)?.mechanism?.diagnostic ?: "native-packaging-unavailable",
        )
        // TODO(phase 6): successful packaging preflight must also prove the real backend/fence
        // contract and fail closed on missing ABI/assets. Do not populate inventory in isolation.
        return owner.open(launch, port, initiallyForeground, monotonicMs, onEvent, onKeepAwake)
    }
}
