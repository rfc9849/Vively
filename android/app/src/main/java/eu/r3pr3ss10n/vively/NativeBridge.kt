package eu.r3pr3ss10n.vively

import android.content.Context

/** Plain JNI entry points in libvively_ffi that UniFFI/JNA don't cover. */
object NativeBridge {
    init {
        System.loadLibrary("vively_ffi")
    }

    /** Must run before the core opens audio (cpal needs the JavaVM + Context). */
    @JvmStatic
    external fun initAndroidContext(context: Context)
}
