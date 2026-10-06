package com.loveoverflow.tabula.mobile.preview

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.loveoverflow.tabula.mobile.bridge.HostMessage

/**
 * A stand-in for the game document, for desktop preview and tests only.
 *
 * It speaks the page side of the ADR-0033 bridge: it says `hello`, answers `init` with `ready` and a
 * keep-awake request, reacts to suspend/resume/back/dispose and can be told to fail or leave. It
 * writes page messages as JSON text exactly like `host-bridge.js`, so the shell's real
 * `GameSession` decodes them through the real codec. It draws no game and says nothing about the
 * real document, which is covered separately (`tools/mobile-host-check`).
 */
internal class SimulatedPage(private val sendToHost: (String) -> Unit) {
    enum class Phase { NotStarted, Loading, Ready, Failed, Left, Retired }

    // Observable so the preview recomposes when the host changes the page, as a real page would repaint.
    var phase: Phase by mutableStateOf(Phase.NotStarted)
        private set
    // Observable so the preview recomposes when the host changes the page, as a real page would repaint.
    var generation: Int by mutableStateOf(0)
        private set
    // Observable so the preview recomposes when the host changes the page, as a real page would repaint.
    var suspended: Boolean by mutableStateOf(false)
        private set
    // Observable so the preview recomposes when the host changes the page, as a real page would repaint.
    var leaveDialogOpen: Boolean by mutableStateOf(false)
        private set
    // Observable so the preview recomposes when the host changes the page, as a real page would repaint.
    var keepAwakeGranted: Boolean? by mutableStateOf(null)
        private set
    val received = mutableListOf<HostMessage>()
    private var requestId = 0

    fun summary() = "phase=$phase gen=$generation suspended=$suspended leaveDialog=$leaveDialogOpen awake=$keepAwakeGranted"

    /** A document load: the page's first message, and again for each reload. */
    fun load() {
        phase = Phase.Loading
        leaveDialogOpen = false
        keepAwakeGranted = null
        generation = 0
        send("""{"v":1,"type":"hello"}""")
    }

    /** The WASM finished starting. Does nothing unless the host has initialised this document. */
    fun completeBoot(bootMs: Int = 900) {
        if (phase != Phase.Loading || generation == 0) return
        phase = Phase.Ready
        send("""{"v":1,"type":"ready","gen":$generation,"bootMs":$bootMs}""")
        requestId += 1
        send("""{"v":1,"type":"service","gen":$generation,"id":$requestId,"name":"keep-awake","enabled":true}""")
    }

    fun fail(detail: String = "simulated page failure") {
        if (phase == Phase.Retired) return
        phase = Phase.Failed
        send("""{"v":1,"type":"failed","gen":$generation,"code":"runtime","detail":"$detail"}""")
    }

    /** The page's own Try again: a reload, which is a new document. */
    fun reload() = load()

    fun openLeaveDialog() {
        if (phase == Phase.Ready) leaveDialogOpen = true
    }

    fun confirmLeave() {
        if (phase != Phase.Ready || !leaveDialogOpen) return
        phase = Phase.Left
        send("""{"v":1,"type":"exit","gen":$generation}""")
    }

    fun dismissLeaveDialog() {
        leaveDialogOpen = false
    }

    /** What the real page does with each host message. */
    fun onHost(message: HostMessage) {
        if (phase == Phase.Retired) return
        received += message
        when (message) {
            is HostMessage.Init -> {
                generation = message.generation
            }
            is HostMessage.Suspend -> suspended = true
            is HostMessage.Resume -> suspended = false
            is HostMessage.BackRequested -> if (phase == Phase.Ready) leaveDialogOpen = !leaveDialogOpen
            is HostMessage.Dispose -> phase = Phase.Retired
            is HostMessage.Reply -> if (message.id == requestId) keepAwakeGranted = message.ok
        }
    }

    private fun send(text: String) {
        if (phase == Phase.Retired) return
        sendToHost(text)
    }
}
