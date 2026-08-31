package com.onyx

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable

/**
 * Minimal skeleton entry point for A1 -- proves the Compose baseline
 * builds and renders. Real screens (Dashboard, Missions, Tasks, ...)
 * are A4's scope, not this one.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            OnyxSkeletonScreen()
        }
    }
}

@Composable
fun OnyxSkeletonScreen() {
    MaterialTheme {
        Surface {
            Text("ONYX Android (A1 skeleton)")
        }
    }
}
