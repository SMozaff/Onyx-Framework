package com.onyx.util

import android.content.Context
import android.content.SharedPreferences
import android.content.res.Configuration
import java.util.Locale

object LocaleHelper {
    private const val PREFS = "onyx_prefs"
    private const val KEY_LOCALE = "locale"

    @JvmStatic
    fun getStoredLocale(context: Context): String {
        val prefs: SharedPreferences = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val stored = prefs.getString(KEY_LOCALE, null)
        return if (stored == "fa" || stored == "en") stored else "en"
    }

    @JvmStatic
    fun setLocale(context: Context, locale: String) {
        val prefs: SharedPreferences = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        prefs.edit().putString(KEY_LOCALE, locale).apply()
        updateResources(context, locale)
    }

    @JvmStatic
    fun applyFromStorage(context: Context) {
        updateResources(context, getStoredLocale(context))
    }

    private fun updateResources(context: Context, locale: String) {
        val target = if (locale == "fa") Locale("fa", "IR") else Locale.ENGLISH
        Locale.setDefault(target)

        val config = Configuration(context.resources.configuration)
        config.setLocale(target)
        config.setLayoutDirection(target)
        context.resources.updateConfiguration(config, context.resources.displayMetrics)
    }
}
