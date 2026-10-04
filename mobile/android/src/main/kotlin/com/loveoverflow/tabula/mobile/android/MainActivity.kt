package com.loveoverflow.tabula.mobile.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import com.loveoverflow.tabula.mobile.installTabulaContent

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        installTabulaContent()
    }
}
