package com.kashikume.feather.testapp

import android.os.Bundle
import android.view.WindowManager
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.androidgamesdk.GameActivity

/**
 * The test app's activity. GameActivity loads the Rust library named in the manifest
 * (`android.app.lib_name`) and runs its `android_main`; this class only sets up the window:
 * full screen, kept on, and back leaving the app.
 */
class MainActivity : GameActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        hideSystemBars()

        // Back (button or gesture) ends the app: the Rust side ends its event loop (winit 0.30
        // doesn't on its own when the activity is destroyed), destroys the engine and ends the
        // process, and the activity finishes. Apps targeting Android 16+ get back only through
        // these callbacks, not as a key; and Android's default for the root activity would be
        // to move the app to the background instead.
        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    nativeRequestExit()
                    finish()
                }
            },
        )
    }

    /** In the Rust library (`android.rs`): asks the engine's event loop to end. */
    private external fun nativeRequestExit()

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // The bars come back with dialogs and notifications; hide them again.
        if (hasFocus) {
            hideSystemBars()
        }
    }

    /** Hides the status and navigation bars; a swipe from the edge shows them for a moment. */
    private fun hideSystemBars() {
        val controller = WindowCompat.getInsetsController(window, window.decorView)
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller.hide(WindowInsetsCompat.Type.systemBars())
    }
}
