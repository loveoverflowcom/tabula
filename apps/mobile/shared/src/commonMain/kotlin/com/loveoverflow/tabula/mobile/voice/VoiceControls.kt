package com.loveoverflow.tabula.mobile.voice

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaShape
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.design.TabulaText
import com.loveoverflow.tabula.mobile.design.TabulaType
import com.loveoverflow.tabula.mobile.shell.ShellButton

/** Compact CMP-owned controls using the existing semantic theme; no WebView microphone UI. */
@Composable
fun VoiceControls(controller: VoiceController, vietnamese: Boolean, modifier: Modifier = Modifier) {
    val state = controller.state
    val colors = LocalTabulaColors.current
    val active = state.connection in setOf(VoiceConnection.CONNECTED, VoiceConnection.RECONNECTING, VoiceConnection.CONNECTING, VoiceConnection.RESOLVING_GRANT)
    fun copy(en: String, vi: String) = if (vietnamese) vi else en
    val status = when (state.connection) {
        VoiceConnection.IDLE -> copy("Voice off", "Voice đang tắt")
        VoiceConnection.RESOLVING_GRANT -> copy("Checking voice access…", "Đang kiểm tra quyền voice…")
        VoiceConnection.CONNECTING -> copy("Connecting voice…", "Đang kết nối voice…")
        VoiceConnection.CONNECTED -> if (state.microphoneEnabled) copy("Voice connected · Mic on", "Đã kết nối voice · Mic bật") else copy("Voice connected · Mic off", "Đã kết nối voice · Mic tắt")
        VoiceConnection.RECONNECTING -> copy("Reconnecting voice…", "Đang kết nối lại voice…")
        VoiceConnection.UNAVAILABLE -> copy("Voice unavailable: backend grants are not connected", "Voice chưa khả dụng: backend chưa cấp grant")
        VoiceConnection.FAILED -> copy("Voice stopped", "Voice đã dừng")
    }
    Column(
        modifier.fillMaxWidth().background(colors.container, RoundedCornerShape(TabulaShape.card.dp))
            .padding(TabulaSpace.md.dp),
        verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp),
    ) {
        TabulaText(status, TabulaType.labelLg, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        val detail = when (state.error) {
            VoiceError.PERMISSION_DENIED -> copy("Microphone permission denied. You can still listen; enable permission in system settings to speak.", "Quyền mic bị từ chối. Bạn vẫn nghe được; bật quyền trong cài đặt hệ thống để nói.")
            VoiceError.GRANT_EXPIRED -> copy("Voice grant expired. Join again to request current access.", "Grant voice đã hết hạn. Tham gia lại để yêu cầu quyền mới.")
            VoiceError.CONNECTION_FAILED -> copy("Could not connect. Check the network and join again.", "Không thể kết nối. Kiểm tra mạng rồi tham gia lại.")
            VoiceError.PUBLICATION_FAILED -> copy("Microphone could not start. Try again.", "Không thể bật mic. Hãy thử lại.")
            VoiceError.AUDIO_INTERRUPTED -> copy("Audio was interrupted. Join again when ready.", "Âm thanh bị gián đoạn. Tham gia lại khi sẵn sàng.")
            VoiceError.BACKGROUND_STOPPED -> copy("Voice stopped in the background. Join again to resume; your mic stays off.", "Voice dừng khi chuyển nền. Tham gia lại để tiếp tục; mic vẫn tắt.")
            else -> null
        }
        if (detail != null) TabulaText(detail, TabulaType.bodyMd, color = colors.onSurfaceVariant)
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp),
            verticalArrangement = Arrangement.spacedBy(TabulaSpace.sm.dp),
        ) {
            ShellButton(
                if (active) copy("Leave voice", "Rời voice") else copy("Join voice", "Tham gia voice"),
                filled = !active,
                onClick = { if (active) controller.leaveVoice() else controller.join() },
            )
            if (state.connection == VoiceConnection.CONNECTED && state.canPublish) {
                ShellButton(
                    if (state.microphoneBusy) copy("Changing mic…", "Đang đổi mic…")
                    else if (state.microphoneEnabled) copy("Turn mic off", "Tắt mic") else copy("Turn mic on", "Bật mic"),
                    filled = state.microphoneEnabled,
                    enabled = !state.microphoneBusy,
                    onClick = { controller.setMicrophoneEnabled(!state.microphoneEnabled) },
                )
            }
        }
        TabulaText(copy("Local mic control. Voice permissions are enforced by the server/SFU.", "Nút mic chỉ điều khiển thiết bị. Server/SFU thực thi quyền voice."), TabulaType.bodyMd, color = colors.onSurfaceVariant)
    }
}
