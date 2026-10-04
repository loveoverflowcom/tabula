package com.loveoverflow.tabula.mobile

import androidx.compose.ui.window.ComposeUIViewController
import platform.UIKit.UIViewController

/** The view controller the Swift host presents; the host stays a thin container. */
fun TabulaViewController(): UIViewController = ComposeUIViewController { TabulaApp() }
