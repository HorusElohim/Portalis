//! Android's native gateway to MediaStore and Storage Access Framework.
//!
//! Rust owns this gateway because torrent piece I/O runs in Rust. Flutter may
//! request a URI in the future, but it must never relay file bytes or turn a
//! URI into a cache path. The installed application context will be used by
//! the Android `ContentLocation` storage adapter.

use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JObject};
use jni::JNIEnv;
use jni::JavaVM;

static JVM: OnceLock<JavaVM> = OnceLock::new();
static APPLICATION_CONTEXT: OnceLock<GlobalRef> = OnceLock::new();

/// Called exactly once by `PortalisNative.install` during Android activity
/// startup. Keeping the application context, not an Activity, prevents a
/// rotation from invalidating long-lived torrent I/O.
#[no_mangle]
pub extern "system" fn Java_com_example_portalis_PortalisNative_installContext(
    env: JNIEnv,
    _class: JClass,
    context: JObject,
) {
    let installed = (|| -> jni::errors::Result<()> {
        let vm = env.get_java_vm()?;
        let context = env.new_global_ref(context)?;
        let _ = JVM.set(vm);
        let _ = APPLICATION_CONTEXT.set(context);
        Ok(())
    })();
    if let Err(error) = installed {
        crate::log::clog!("android_content", "could not install Android context: {error}");
    }
}
