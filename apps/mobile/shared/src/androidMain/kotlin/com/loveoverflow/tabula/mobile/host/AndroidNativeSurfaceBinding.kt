package com.loveoverflow.tabula.mobile.host

import android.os.Looper
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View

/**
 * Opaque SurfaceHolder-owned surface. The backend must acquire/release its own native window
 * reference; this wrapper never releases the Android Surface owned by the view (ADR-0043).
 */
class AndroidNativeSurface internal constructor(val surface: Surface) : NativeSurface

/** Checks the owner thread for NativeRuntimeOwner and native callbacks; no silent thread hopping. */
fun assertAndroidNativeOwnerThread() {
    check(Looper.myLooper() == Looper.getMainLooper()) { "Native GameHost must run on the Android main thread" }
}

/**
 * Real Android callbacks for a dedicated game SurfaceView, with an injected native control port.
 * This is NOT a Macroquad backend/production selector and never reports Ready itself. A future
 * Android GameHost may embed this view through AndroidView in the existing CMP screen.
 *
 * Coordinates/dimensions are local physical pixels. CMP owns safe insets. Render scheduling,
 * context, decoding/upload and first usable frame stay in Rust; there is no Kotlin frame ticker.
 * Native callbacks must be marshalled to main before invoking the supplied runtime callback.
 * Detach must fence render-thread access before surfaceDestroyed returns, as SurfaceHolder
 * requires; Stop acknowledgement additionally joins the worker before another launch.
 */
class AndroidNativeSurfaceBinding(
    private val view: SurfaceView,
    private val runtime: NativeGameRuntime,
) : SurfaceHolder.Callback, View.OnTouchListener {
    private var surface: AndroidNativeSurface? = null
    private var closed = false

    init {
        assertAndroidNativeOwnerThread()
        view.holder.addCallback(this)
        view.setOnTouchListener(this)
        // SurfaceView may already be attached when the binding enters composition.
        if (view.holder.surface.isValid) {
            surfaceCreated(view.holder)
            surfaceChanged(view.holder, 0, view.holder.surfaceFrame.width(), view.holder.surfaceFrame.height())
        }
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        assertAndroidNativeOwnerThread()
        if (closed || holder !== view.holder || !holder.surface.isValid) return
        detachCurrent()
        surface = AndroidNativeSurface(holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        assertAndroidNativeOwnerThread()
        if (closed || holder !== view.holder) return
        val current = surface ?: return
        val geometry = NativeSurfaceGeometry.create(width, height, view.resources.displayMetrics.density)
        if (geometry == null || !current.surface.isValid) {
            detachCurrent()
            return
        }
        val lease = runtime.leaseFor(current)
        if (lease == null) runtime.attach(current, geometry) else runtime.resize(lease, geometry)
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        assertAndroidNativeOwnerThread()
        if (closed || holder !== view.holder) return
        detachCurrent()
        surface = null
    }

    override fun onTouch(target: View, event: MotionEvent): Boolean {
        assertAndroidNativeOwnerThread()
        if (closed || target !== view) return false
        val lease = surface?.let(runtime::leaseFor) ?: return false
        if (event.pointerCount !in 1..10) {
            runtime.cancelPointers(lease)
            return true
        }
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> forward(event, event.actionIndex, NativePointer.Phase.Down, lease)
            MotionEvent.ACTION_MOVE -> for (index in 0 until event.pointerCount) forward(event, index, NativePointer.Phase.Move, lease)
            MotionEvent.ACTION_UP, MotionEvent.ACTION_POINTER_UP -> {
                forward(event, event.actionIndex, NativePointer.Phase.Up, lease)
                if (event.actionMasked == MotionEvent.ACTION_UP) view.performClick()
            }
            MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_OUTSIDE -> runtime.cancelPointers(lease)
            else -> return false
        }
        return true
    }

    private fun forward(event: MotionEvent, index: Int, phase: NativePointer.Phase, lease: NativeSurfaceLease) {
        if (index !in 0 until event.pointerCount) {
            runtime.cancelPointers(lease)
            return
        }
        runtime.pointer(lease, NativePointer(event.getPointerId(index), phase, event.getX(index), event.getY(index)))
    }

    /** Remove view callbacks and retire this surface; the GameRuntimeControls binding owns Stop. */
    fun close() {
        assertAndroidNativeOwnerThread()
        if (closed) return
        closed = true
        view.holder.removeCallback(this)
        view.setOnTouchListener(null)
        detachCurrent()
        surface = null
    }

    private fun detachCurrent() {
        surface?.let(runtime::leaseFor)?.let(runtime::detach)
    }
}
