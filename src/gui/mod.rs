//! GUI module for Kunai providing a GNOME Files (Nautilus) inspired PKG browser.

pub mod app;
pub mod preview;
pub mod theme;
pub mod widgets;

pub use app::{KunaiApp, run_gui};
