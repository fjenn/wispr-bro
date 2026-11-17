//! Platform-specific keyboard utilities
//!
//! Wispr Bro (iOS): no platform backend. An iPhone app has no global
//! keyboard hook; `KeyboardListener::new` reports `Error::Platform`.

pub(crate) mod state;
