package com.loveoverflow.tabula.mobile.voice

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import kotlinx.coroutines.CompletableDeferred

/** Activity-owned permission prompt, registered before setContent/start (ADR-0037). */
internal class AndroidMicrophonePermission(private val activity: ComponentActivity) {
    private var pending: CompletableDeferred<Boolean>? = null
    private var closed = false
    private val launcher = activity.registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        val request = pending
        pending = null
        if (!closed) request?.complete(granted)
    }

    suspend fun request(): Boolean {
        if (closed) return false
        if (activity.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) return true
        // A cancelled coroutine cannot cancel the OS dialog. Keep its slot until the old result
        // arrives; it must never become permission for a newer attempt/room.
        if (pending != null) return false
        val request = CompletableDeferred<Boolean>()
        pending = request
        try {
            launcher.launch(Manifest.permission.RECORD_AUDIO)
            return request.await()
        } catch (cancelled: kotlinx.coroutines.CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            if (pending === request) pending = null
            return false
        } finally {
            if (!request.isCompleted) request.cancel()
        }
    }

    fun close() {
        if (closed) return
        closed = true
        pending?.cancel()
        pending = null
        launcher.unregister()
    }
}
