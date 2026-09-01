//! rmail-mobile
//!
//! Placeholder FFI layer that will expose `rmail-core` to native iOS/Android
//! UIs via UniFFI-generated bindings.

/// Placeholder re-export to confirm the mobile layer links against core.
pub fn core_version() -> String {
    rmail_core::version().to_string()
}
