package com.onyx.util;

import android.content.Context;
import android.content.SharedPreferences;
import android.content.res.Configuration;
import java.util.Locale;

public final class LocaleHelper {
    private static final String PREFS = "onyx_prefs";
    private static final String KEY_LOCALE = "locale";

    public static String getStoredLocale(Context context) {
        SharedPreferences prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        String stored = prefs.getString(KEY_LOCALE, null);
        return (stored != null && (stored.equals("fa") || stored.equals("en"))) ? stored : "en";
    }

    public static void setLocale(Context context, String locale) {
        SharedPreferences prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
        prefs.edit().putString(KEY_LOCALE, locale).apply();
        updateResources(context, locale);
    }

    public static void applyFromStorage(Context context) {
        String stored = getStoredLocale(context);
        updateResources(context, stored);
    }

    private static void updateResources(Context context, String locale) {
        Locale target = locale.equals("fa") ? new Locale("fa", "IR") : Locale.ENGLISH;
        Locale.setDefault(target);
        Configuration config = context.getResources().getConfiguration();
        config.setLocale(target);
        config.setLayoutDirection(target);
        context.getResources().updateConfiguration(config, context.getResources().getDisplayMetrics());
    }

    private LocaleHelper() {}
}
