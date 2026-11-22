//! The Android FFI layer runs `AppCore` on a dedicated actor thread, so the
//! core must be movable across threads.

fn assert_send<T: Send>() {}

#[test]
fn app_core_is_send() {
    assert_send::<vively_app::AppCore>();
    assert_send::<vively_app::CoreEvent>();
}
