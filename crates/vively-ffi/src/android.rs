//! Registers the JavaVM + application Context with `ndk-context`, which cpal's
//! AAudio backend uses for device enumeration. Normally a NativeActivity does
//! this; the core is loaded through JNA, so Kotlin calls this once instead.

use std::{ffi::c_void, ptr, sync::Once};

use jni_sys::{JNIEnv, JavaVM, jclass, jobject};

static INIT: Once = Once::new();

/// `eu.r3pr3ss10n.vively.NativeBridge.initAndroidContext(Context)`
#[unsafe(no_mangle)]
pub extern "system" fn Java_eu_r3pr3ss10n_vively_NativeBridge_initAndroidContext(
    env: *mut JNIEnv,
    _class: jclass,
    context: jobject,
) {
    INIT.call_once(|| unsafe {
        let iface = &**env;
        let mut vm: *mut JavaVM = ptr::null_mut();
        let get_vm = iface.GetJavaVM.expect("JNIEnv::GetJavaVM");
        if get_vm(env, &mut vm) != 0 || vm.is_null() {
            return;
        }
        // Global ref: the context must outlive this call.
        let new_global = iface.NewGlobalRef.expect("JNIEnv::NewGlobalRef");
        let global = new_global(env, context);
        if global.is_null() {
            return;
        }
        ndk_context::initialize_android_context(vm as *mut c_void, global as *mut c_void);
    });
}
