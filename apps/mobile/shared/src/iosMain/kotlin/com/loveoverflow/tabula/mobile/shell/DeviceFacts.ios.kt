package com.loveoverflow.tabula.mobile.shell

import androidx.compose.runtime.Composable
import platform.Foundation.NSLocale
import platform.Foundation.preferredLanguages
import platform.UIKit.UIAccessibilityIsReduceMotionEnabled

/** iOS "Reduce Motion" and the first preferred language. */
@Composable
actual fun rememberDeviceFacts(): DeviceFacts = DeviceFacts(
    reducedMotion = UIAccessibilityIsReduceMotionEnabled(),
    languageTag = (NSLocale.preferredLanguages.firstOrNull() as? String) ?: "en",
)
