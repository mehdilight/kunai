//! Platform abstraction layer using the Strategy Pattern.
//!
//! Provides extensible operating system support, focusing primarily on macOS
//! while allowing seamless addition of Linux, Windows, or BSD systems.

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::core::pkg::PkgType;

pub mod linux;
pub mod macos;
pub mod windows;

pub use linux::LinuxPlatform;
pub use macos::MacOsPlatform;
pub use windows::WindowsPlatform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsType {
    MacOs,
    Linux,
    Windows,
    Generic,
}

/// Strategy trait for OS-specific directories, system integration, and actions.
pub trait Platform: Send + Sync {
    /// Human-readable name of the platform.
    fn name(&self) -> &'static str;

    /// The OS category.
    fn os_type(&self) -> OsType;

    /// Default base directory for shadPS4 on this platform.
    fn default_shadps4_dir(&self) -> Option<PathBuf>;

    /// Default directory where shadPS4 installs base games and patches.
    fn default_games_dir(&self) -> Option<PathBuf>;

    /// Default directory where shadPS4 installs DLCs / add-on content.
    fn default_addons_dir(&self) -> Option<PathBuf>;

    /// Resolves the destination directory for extracting a given PKG type.
    fn resolve_target_dir(
        &self,
        pkg_type: PkgType,
        title_id: &str,
        entitlement_label: Option<&str>,
        custom_games_dir: Option<&Path>,
        custom_addons_dir: Option<&Path>,
    ) -> PathBuf {
        let games_dir = custom_games_dir
            .map(|p| p.to_path_buf())
            .or_else(|| self.default_games_dir())
            .unwrap_or_else(|| PathBuf::from("./games"));

        let addons_dir = custom_addons_dir
            .map(|p| p.to_path_buf())
            .or_else(|| self.default_addons_dir())
            .unwrap_or_else(|| PathBuf::from("./addons"));

        match pkg_type {
            PkgType::BaseGame => games_dir.join(title_id),
            PkgType::Patch => games_dir.join(format!("{}-patch", title_id)),
            PkgType::Dlc => {
                let label = entitlement_label.unwrap_or("content");
                addons_dir.join(title_id).join(label)
            }
        }
    }

    /// Opens the specified folder or file in the platform's file manager (e.g. macOS Finder).
    fn reveal_in_file_manager(&self, path: &Path) -> Result<()>;

    /// Dispatches a native desktop notification upon extraction completion.
    fn send_notification(&self, title: &str, message: &str) -> Result<()>;
}

/// Factory function to instantiate the active platform strategy.
pub fn current_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "macos")]
    {
        Box::new(MacOsPlatform::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxPlatform::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsPlatform::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Box::new(MacOsPlatform::new())
    }
}
