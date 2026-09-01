//! rmail-core
//!
//! Shared business logic for RMail, used by both the desktop (Tauri) and
//! mobile (UniFFI) shells. This currently contains only a bare-bones
//! skeleton; sync logic, database schema, and auth will be added here.

pub mod api;
pub mod db;
pub mod services;
/// Placeholder entry point for the core library.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!version().is_empty());
    }
}
