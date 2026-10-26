//! Indexed existing L1 owner; no adoption into fresh Runtime6 or mutable V4 Engine.
use super::{
    admin,
    engine::now,
    handshake::{self, Credentials, Descriptor, Spec},
    state::{self, Head, PrivateState},
};
use anyhow::{Context, Result, ensure};
use kilogram_protocol::ordered_dialog_v1::{self as wire, Event, Frame, Id, Value, a, b, opt, u};
use kilogram_state::indexed_dialog::{self as db, migration7::live1 as store};
mod config;
pub use config::Profile3;
mod bootstrap;
mod data;
mod network;
mod owner;
mod protocol;
mod resources;
pub mod runtime;
use super::runtime2::window;
#[cfg(test)]
mod review1_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_l1_ref_window_keeps_blocked_context_share() -> Result<()> {
        let window = window::RefWindow2::new(&[0, 1]);
        let blocked = window.reserve(0, 64)?;
        assert!(window.reserve(0, 1).is_err());
        let active = window.reserve(1, 64)?;
        assert!(window.reserve(1, 1).is_err());
        drop(blocked);
        assert!(window.reserve(1, 1).is_err());
        drop(active);
        assert_eq!(window.snapshot(), ([64, 64], [0, 0], [64, 64]));
        assert!(window.reserve(0, 65).is_err());
        Ok(())
    }
}
