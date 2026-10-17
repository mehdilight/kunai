//! Command-line argument definitions using clap.

use clap::{Parser, ValueHint};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "kunai",
    version,
    about = "Kunai (苦無) - Native macOS PlayStation 4 PKG extractor & shadPS4 installer",
    long_about = "A blazing-fast, Apple Silicon native PS4 PKG extractor and shadPS4 package installer.\n\
                  Supports automatic type detection (Base Game, Patch, DLC), cryptographic unpacking,\n\
                  and direct installation into macOS shadPS4 directories."
)]
pub struct Cli {
    /// Path to the .pkg file or directory (when using --batch).
    #[arg(value_hint = ValueHint::FilePath)]
    pub pkg_path: Option<PathBuf>,

    /// Destination directory for extracted contents.
    /// If not provided and --install is not specified, extracts next to the PKG file.
    #[arg(value_hint = ValueHint::DirPath)]
    pub output_dir: Option<PathBuf>,

    /// Check PKG type and exit immediately:
    /// 101 = Base Game, 102 = Game Update / Patch, 103 = DLC / Add-on.
    #[arg(long, help = "Detect and return PKG type exit code (101=game, 102=patch, 103=dlc)")]
    pub check_type: bool,

    /// Automatically install directly into the shadPS4 directory based on PKG type.
    #[arg(short = 'i', long, help = "Install directly into shadPS4 directory")]
    pub install: bool,

    /// Batch mode: extract all .pkg files found in the given folder.
    #[arg(short = 'b', long, help = "Batch process all .pkg files in a directory")]
    pub batch: bool,

    /// Inspect and print detailed PKG metadata and param.sfo information.
    #[arg(long, help = "Print detailed package metadata without extracting")]
    pub info: bool,

    /// Reveal the extracted directory in Finder (macOS) or system file manager upon completion.
    #[arg(short = 'r', long, help = "Reveal output in Finder after extraction")]
    pub reveal: bool,

    /// Send a native system notification and chime upon completion.
    #[arg(long, help = "Send desktop notification when extraction completes")]
    pub notify: bool,

    /// Override custom games directory.
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub games_dir: Option<PathBuf>,

    /// Override custom addons directory.
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub addons_dir: Option<PathBuf>,
}
