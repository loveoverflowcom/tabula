package com.loveoverflow.tabula.mobile.preview

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Paint
import com.loveoverflow.tabula.mobile.account.AccountIdentity
import com.loveoverflow.tabula.mobile.design.TabulaScheme
import com.loveoverflow.tabula.mobile.design.TabulaSchemes
import com.loveoverflow.tabula.mobile.shell.AccountAvatarImage

/**
 * An actual in-memory bitmap, explicitly synthetic and already managed by this desktop fixture.
 * No URL, remote image loader or real person's photo is admitted. Shared UI must fall back to its
 * neutral silhouette when this exact identity snapshot is absent or replaced.
 */
internal fun syntheticManagedAvatar(identity: AccountIdentity, scheme: TabulaScheme): AccountAvatarImage {
    val colors = TabulaSchemes.of(scheme)
    val image = ImageBitmap(64, 64)
    val canvas = Canvas(image)
    canvas.drawRect(Rect(0f, 0f, 64f, 64f), Paint().apply { color = colors.containerHigh })
    val ink = Paint().apply { color = colors.onSurfaceVariant }
    canvas.drawCircle(Offset(32f, 22f), 10f, ink)
    canvas.drawCircle(Offset(32f, 56f), 22f, ink)
    return AccountAvatarImage(identity, image)
}
