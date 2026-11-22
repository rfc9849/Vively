package eu.r3pr3ss10n.vively

import android.content.Context
import android.os.Build
import androidx.annotation.RequiresApi
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** Android-only appearance options, stored outside the shared core settings. */
class ThemePrefs(context: Context) {
    private val prefs = context.getSharedPreferences("theme", Context.MODE_PRIVATE)

    /** Accent from the system (Material You) palette instead of the playing cover. */
    var systemColors by mutableStateOf(supported && prefs.getBoolean(KEY_SYSTEM_COLORS, true))
        private set

    fun updateSystemColors(on: Boolean) {
        systemColors = on && supported
        prefs.edit().putBoolean(KEY_SYSTEM_COLORS, systemColors).apply()
    }

    companion object {
        private const val KEY_SYSTEM_COLORS = "system_colors"

        /** Dynamic colours exist from Android 12. */
        val supported: Boolean get() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

        @RequiresApi(Build.VERSION_CODES.S)
        fun systemAccentHex(context: Context): String =
            String.format("#%06x", context.getColor(android.R.color.system_accent1_500) and 0xFFFFFF)
    }
}
