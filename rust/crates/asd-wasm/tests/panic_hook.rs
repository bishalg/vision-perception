//! owner: WP-R.6
//!
//! The panic hook keeps the message for `lastPanic()` (its own test binary: the hook is
//! process-wide).

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn panic_message_is_kept_for_last_panic() {
    assert_eq!(asd_wasm::last_panic(), None);
    asd_wasm::install_panic_hook();
    let caught = std::panic::catch_unwind(|| panic!("asd-wasm test panic"));
    assert!(caught.is_err());
    let message = asd_wasm::last_panic().expect("recorded");
    assert!(message.contains("asd-wasm test panic"), "{message}");
}
