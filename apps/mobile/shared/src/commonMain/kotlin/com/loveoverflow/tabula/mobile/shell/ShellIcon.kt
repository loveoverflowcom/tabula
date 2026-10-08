package com.loveoverflow.tabula.mobile.shell

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.loveoverflow.tabula.mobile.design.TabulaSpace
import com.loveoverflow.tabula.mobile.resources.Res
import com.loveoverflow.tabula.mobile.resources.material_symbol_arrow_back
import com.loveoverflow.tabula.mobile.resources.material_symbol_grid_view
import com.loveoverflow.tabula.mobile.resources.material_symbol_home
import com.loveoverflow.tabula.mobile.resources.material_symbol_person
import com.loveoverflow.tabula.mobile.resources.material_symbol_radio_button_checked
import com.loveoverflow.tabula.mobile.resources.material_symbol_radio_button_unchecked
import com.loveoverflow.tabula.mobile.resources.material_symbol_search
import org.jetbrains.compose.resources.painterResource

/** Official Material Symbols used by shell controls; game and brand artwork have separate owners. */
internal enum class ShellSymbol { Home, Library, Account, Back, Search, RadioChecked, RadioUnchecked }

/**
 * A decorative Material3 icon; its parent owns the localized action/name/state (doc 04 §10).
 * Tint and size come from Tabula tokens. Back retains the official vector's RTL auto-mirroring.
 */
@Composable
internal fun ShellIcon(
    symbol: ShellSymbol,
    color: Color,
    modifier: Modifier = Modifier,
    size: Dp = TabulaSpace.xxl.dp,
) {
    val resource = when (symbol) {
        ShellSymbol.Home -> Res.drawable.material_symbol_home
        ShellSymbol.Library -> Res.drawable.material_symbol_grid_view
        ShellSymbol.Account -> Res.drawable.material_symbol_person
        ShellSymbol.Back -> Res.drawable.material_symbol_arrow_back
        ShellSymbol.Search -> Res.drawable.material_symbol_search
        ShellSymbol.RadioChecked -> Res.drawable.material_symbol_radio_button_checked
        ShellSymbol.RadioUnchecked -> Res.drawable.material_symbol_radio_button_unchecked
    }
    Icon(
        painter = painterResource(resource),
        contentDescription = null,
        tint = color,
        modifier = modifier.size(size).clearAndSetSemantics { },
    )
}
