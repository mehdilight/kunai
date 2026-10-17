//! Windows platform implementation.
//!
//! Handles Windows `%APPDATA%\shadPS4` directory paths and explorer integration.

use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{OsType, Platform};

#[derive(Debug, Clone, Default)]
pub struct WindowsPlatform {
    base_shadps4_dir: Option<PathBuf>,
}

impl WindowsPlatform {
    pub fn new() -> Self {
        let base_shadps4_dir = env::var("APPDATA").ok().map(|appdata| PathBuf::from(appdata).join("shadPS4"));
        Self { base_shadps4_dir }
    }
}

impl Platform for WindowsPlatform {
    fn name(&self) -> &'static str {
        "Windows"
    }

    fn os_type(&self) -> OsType {
        OsType::Windows
    }

    fn default_shadps4_dir(&self) -> Option<PathBuf> {
        self.base_shadps4_dir.clone()
    }

    fn default_games_dir(&self) -> Option<PathBuf> {
        self.base_shadps4_dir.as_ref().map(|b| b.join("games"))
    }

    fn default_addons_dir(&self) -> Option<PathBuf> {
        self.base_shadps4_dir.as_ref().map(|b| b.join("user").join("addcont"))
    }

    fn reveal_in_file_manager(&self, path: &Path) -> Result<()> {
        let _ = Command::new("explorer.exe")
            .arg(format!("/select,\"{}\"", path.display()))
            .spawn();
        Ok(())
    }

    fn send_notification(&self, _title: &str, _message: &str) -> Result<()> {
        // Optional Windows toast or sound
        Ok(())
    }
}
