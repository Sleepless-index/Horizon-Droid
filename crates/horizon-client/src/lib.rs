//! Gameplay and startup, above the reusable client library.
#![recursion_limit = "256"]
#![cfg_attr(not(target_os = "android"), forbid(unsafe_code))]
extern crate self as horizon_client;
pub use horizon_client_lib::*;
pub mod mod_api;
