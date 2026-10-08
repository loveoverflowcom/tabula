package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.account.AccountIdentity
import com.loveoverflow.tabula.mobile.design.LocalTabulaColors
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.localization.AccountCopy
import com.loveoverflow.tabula.mobile.localization.ShellCopy
import com.loveoverflow.tabula.mobile.localization.ShellStrings
import com.loveoverflow.tabula.mobile.localization.account

/**
 * An already-loaded, permitted managed image bound to one exact identity presentation snapshot.
 * The platform resource owner validates/loads the bitmap before supplying it (doc 04 §1.2).
 * There is no avatar URL in the current HTTP DTO, no loader here, and no production image source.
 * Reference identity prevents an old completion from repainting a refreshed or switched account.
 */
class AccountAvatarImage(val identity: AccountIdentity, val image: ImageBitmap) {
    override fun toString(): String = "AccountAvatarImage(<managed image>)"
}

/** Shared account/header silhouette and managed crop, without initials, status or seat-derived art. */
@Composable
fun ShellIdentityAvatar(
    identity: AccountIdentity?,
    image: AccountAvatarImage?,
    strings: ShellStrings,
    modifier: Modifier = Modifier,
    size: Dp = TabulaSpace.xxxxxl.dp,
    decorative: Boolean = false,
) {
    val colors = LocalTabulaColors.current
    val currentImage = image?.takeIf { identity != null && it.identity === identity }
    val semantics = if (decorative) Modifier.clearAndSetSemantics { } else Modifier.semantics {
        contentDescription = if (identity == null) strings[ShellCopy.Anonymous]
        else strings.account(if (currentImage == null) AccountCopy.NeutralAvatar else AccountCopy.Avatar)
    }
    Box(modifier.size(size).clip(CircleShape).background(colors.shellHero).then(semantics)) {
        if (currentImage == null) {
            NeutralAvatar(colors.primary, Modifier.size(size).then(if (decorative) Modifier else Modifier.testTag("account-avatar-neutral")), size)
        } else {
            Image(currentImage.image, contentDescription = null,
                modifier = Modifier.size(size).then(if (decorative) Modifier else Modifier.testTag("account-avatar-image")), contentScale = ContentScale.Crop)
        }
    }
}

/** Official neutral Person symbol; no account/seat-derived artwork is inferred (doc 04 §1.2). */
@Composable
internal fun NeutralAvatar(color: Color, modifier: Modifier, size: Dp = TabulaSpace.xxxxxl.dp) {
    ShellIcon(ShellSymbol.Account, color, modifier, size)
}
