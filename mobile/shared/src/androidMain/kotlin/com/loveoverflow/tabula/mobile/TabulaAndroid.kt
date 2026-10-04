package com.loveoverflow.tabula.mobile

import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge

/** Installs the shared Tabula UI into an Android activity; the activity stays a thin host. */
fun ComponentActivity.installTabulaContent() {
    enableEdgeToEdge()
    setContent { TabulaApp() }
}
