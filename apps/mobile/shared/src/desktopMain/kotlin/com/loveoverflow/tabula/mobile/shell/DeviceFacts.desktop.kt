package com.loveoverflow.tabula.mobile.shell

import androidx.compose.runtime.Composable
import java.util.Locale

/**
 * Desktop exists only to preview and test the shell. Reduced motion is an explicit switch
 * (`-Dtabula.preview.reducedMotion=true`) because the JVM has no portable accessibility setting.
 */
@Composable
actual fun rememberDeviceFacts(): DeviceFacts = DeviceFacts(
    reducedMotion = System.getProperty("tabula.preview.reducedMotion") == "true",
    languageTag = System.getProperty("tabula.preview.language") ?: Locale.getDefault().language,
)
