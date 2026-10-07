package com.tabula.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable

@Composable
fun TabulaTheme(
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = TabulaDarkColorScheme,
        typography = TabulaTypography,
        content = content
    )
}
