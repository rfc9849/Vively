package eu.r3pr3ss10n.vively

import android.app.Application
import android.content.Context

class VivelyApplication : Application() {
    lateinit var store: CoreStore
        private set
    lateinit var themePrefs: ThemePrefs
        private set

    override fun onCreate() {
        super.onCreate()
        NativeBridge.initAndroidContext(applicationContext)
        store = CoreStore(this)
        themePrefs = ThemePrefs(this)
    }
}

val Context.coreStore: CoreStore
    get() = (applicationContext as VivelyApplication).store

val Context.themePrefs: ThemePrefs
    get() = (applicationContext as VivelyApplication).themePrefs
