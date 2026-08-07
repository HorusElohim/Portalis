//! Android's native gateway to MediaStore and Storage Access Framework.
//!
//! Rust owns this gateway because torrent piece I/O runs in Rust. Flutter may
//! request a URI in the future, but it must never relay file bytes or turn a
//! URI into a cache path. The installed application context will be used by
//! the Android `ContentLocation` storage adapter.

use std::sync::OnceLock;

use jni::objects::{Global, JClass, JObject};
use jni::vm::JavaVM;
use jni::EnvUnowned;

static JVM: OnceLock<JavaVM> = OnceLock::new();
static APPLICATION_CONTEXT: OnceLock<Global<JObject<'static>>> = OnceLock::new();

/// Called exactly once by `PortalisNative.install` during Android activity
/// startup. Keeping the application context, not an Activity, prevents a
/// rotation from invalidating long-lived torrent I/O.
#[no_mangle]
pub extern "system" fn Java_com_example_portalis_PortalisNative_installContext(
    mut env: EnvUnowned<'_>,
    _class: JClass,
    context: JObject,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let vm = env.get_java_vm()?;
        let context = env.new_global_ref(context)?;
        let _ = JVM.set(vm);
        let _ = APPLICATION_CONTEXT.set(context);
        Ok(())
    })
    .resolve::<jni::errors::LogErrorAndDefault>();
}
