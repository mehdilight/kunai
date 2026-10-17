//! Linux platform implementation.
//!
//! Handles Linux XDG specifications (`$XDG_DATA_HOME/shadPS4` or `~/.local/share/shadPS4`),
//! xdg-open, and notify-send.

use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{OsType, Platform};

#[derive(Debug, Clone, Default)]
pub struct LinuxPlatform {
    base_shadps4_dir: Option<PathBuf>,
}

impl LinuxPlatform {
    pub fn new() -> Self {
        let base_shadps4_dir = if let Ok(xdg) = env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                Some(PathBuf::from(xdg).join("shadPS4"))
            } else {
                Self::fallback_dir()
            }
        } else {
            Self::fallback_dir()
        };

        Self { base_shadps4_dir }
    }

    fn fallback_dir() -> Option<PathBuf> {
        env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local").join("share").join("shadPS4"))
    }
}

impl Platform for LinuxPlatform {
    fn name(&self) -> &'static str {
        "Linux"
    }

    fn os_type(&self) -> OsType {
        OsType::Linux
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
        let _ = Command::new("xdg-open").arg(path).spawn();
        Ok(())
    }

    fn send_notification(&self, title: &str, message: &str) -> Result<()> {
        let _ = Command::new("notify-send")
            .arg(title)
            .arg(message)
            .spawn();
        Ok(())
    }
}
