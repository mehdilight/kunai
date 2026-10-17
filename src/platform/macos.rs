//! macOS specialized platform implementation (Apple Silicon & Intel).
//!
//! Handles macOS default paths in `~/Library/Application Support/shadPS4`,
//! automatic configuration parsing, Finder reveal, and system notifications.

use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{OsType, Platform};

#[derive(Debug, Clone, Default)]
pub struct MacOsPlatform {
    base_shadps4_dir: Option<PathBuf>,
}

impl MacOsPlatform {
    pub fn new() -> Self {
        let home = env::var("HOME").ok().map(PathBuf::from);
        let base_shadps4_dir = home.map(|h| h.join("Library").join("Application Support").join("shadPS4"));
        Self { base_shadps4_dir }
    }

    /// Reads shadPS4's config.toml if it exists in Application Support to extract custom paths.
    fn read_config_path(&self, key: &str) -> Option<PathBuf> {
        let base = self.base_shadps4_dir.as_ref()?;
        let config_file = base.join("config.toml");
        if !config_file.exists() {
            return None;
        }

        let content = fs::read_to_string(config_file).ok()?;
        let parsed: toml::Value = toml::from_str(&content).ok()?;

        parsed
            .get("paths")
            .and_then(|p| p.get(key))
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
    }
}

impl Platform for MacOsPlatform {
    fn name(&self) -> &'static str {
        "macOS"
    }

    fn os_type(&self) -> OsType {
        OsType::MacOs
    }

    fn default_shadps4_dir(&self) -> Option<PathBuf> {
        self.base_shadps4_dir.clone()
    }

    fn default_games_dir(&self) -> Option<PathBuf> {
        // Check config.toml first, then fall back to standard ~/Library path
        if let Some(custom) = self.read_config_path("games") {
            return Some(custom);
        }
        self.base_shadps4_dir.as_ref().map(|b| b.join("games"))
    }

    fn default_addons_dir(&self) -> Option<PathBuf> {
        // Check config.toml first, then fall back to standard ~/Library path
        if let Some(custom) = self.read_config_path("addons") {
            return Some(custom);
        }
        self.base_shadps4_dir.as_ref().map(|b| b.join("user").join("addcont"))
    }

    fn reveal_in_file_manager(&self, path: &Path) -> Result<()> {
        Command::new("/usr/bin/open")
            .arg("-R")
            .arg(path)
            .spawn()?
            .wait()?;
        Ok(())
    }

    fn send_notification(&self, title: &str, message: &str) -> Result<()> {
        let script = format!(
            r#"display notification "{}" with title "{}" sound name "Glass""#,
            message.replace('"', "\\\""),
            title.replace('"', "\\\"")
        );

        let _ = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .spawn();

        Ok(())
    }
}
