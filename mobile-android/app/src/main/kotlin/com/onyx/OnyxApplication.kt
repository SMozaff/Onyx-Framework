package com.onyx

import android.app.Application

/**
 * Loads the native `mobile_android_jni` library from the `Application`
 * class specifically, per real, current Android NDK guidance confirmed
 * via Context7 before writing this (developer.android.com/ndk/guides/
 * jni-tips): "For applications with multiple classes using native
 * methods, loading the library from the Application class ensures it
 * is initialized early and consistently." This app will have many such
 * classes as A3/A4 build out real screens and session management, so
 * this is not a premature optimization -- it is the documented-correct
 * pattern for exactly that shape of app, applied from day one rather
 * than retrofitted once a second native-calling class exists.
 *
 * ReLinker is deliberately not used here -- see this module's sibling
 * decision recorded in `DECISIONS.md`'s A1 entry and `app/build.gradle.kts`'s
 * own dependency comment: the loading issues it exists for are
 * documented as affecting API levels below 18, well under this
 * project's real minimum (API 29).
 */
class OnyxApplication : Application() {
    companion object {
        init {
            System.loadLibrary("mobile_android_jni")
        }
    }
}
