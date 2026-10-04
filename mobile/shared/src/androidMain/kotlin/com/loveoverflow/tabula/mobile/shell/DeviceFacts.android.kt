package com.loveoverflow.tabula.mobile.shell

import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext

/** Android's "Remove animations" sets the animator duration scale to zero. */
@Composable
actual fun rememberDeviceFacts(): DeviceFacts {
    val context = LocalContext.current
    // Read on every composition (a cheap cached setting) so a change made while the app is open
    // is honoured by the next game launch; the configuration read keeps language current too.
    val languageTag = LocalConfiguration.current.locales[0]?.language ?: "en"
    val scale = Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
    return DeviceFacts(reducedMotion = scale == 0f, languageTag = languageTag)
}
