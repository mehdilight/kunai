# Kunai (苦無)

[![macOS](https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon%20%26%20Intel)-black)](https://apple.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202021-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue)](LICENSE-MIT)

Kunai (苦無) is a high-performance, native macOS PlayStation 4 PKG extractor and shadPS4 package installer written in Rust.

Named after the traditional Japanese tool used for wedging, prying, and cutting, Kunai decrypts and unpacks PlayStation 4 `.pkg` containers natively on macOS with zero external C/C++ dependencies.

---

## Overview

ShadPS4Plus provided extraction tools primarily packaged for Windows (`.exe`) and Linux (`.AppImage`). Kunai brings complete extraction and decryption parity to macOS with native Apple Silicon optimization, full cryptographic support, and direct integration with shadPS4's directory structure.

### Key Capabilities

* **Apple Silicon Native**: Built for ARM64 (`aarch64-apple-darwin`), taking advantage of ARMv8 hardware cryptography acceleration for AES and SHA256.
* **Full Cryptographic Parity**:
  * Decrypts fake PKG (FPKG) entry keys via RSA-2048 PKCS#1 v1.5 (`dk3` and `ekpfs`).
  * AES-128-CBC decryption for `sce_sys` NP files (`license.dat`, `license.info`, `nptitle.dat`, `npbind.dat`).
  * AES-128-XTS sector decryption for the PFS filesystem and nested files.
  * Inode and Dirent directory tree reconstruction with zlib decompression.
* **Automated shadPS4 Routing**:
  * Automatically resolves `~/Library/Application Support/shadPS4`.
  * Reads shadPS4's `config.toml` to respect custom paths set in emulator preferences.
  * Routes files to their standard locations based on package type:
    * **Base Games** (exit code 101): `games/<TITLE_ID>/`
    * **Game Updates / Patches** (exit code 102): `games/<TITLE_ID>-patch/`
    * **DLCs / Add-on Content** (exit code 103): `user/addcont/<TITLE_ID>/<ENTITLEMENT_LABEL>/`
* **macOS Integration**:
  * `--reveal` (`-r`): Automatically reveals the extracted directory in Finder.
  * `--notify`: Dispatches native macOS notification banners upon task completion.
* **Extensible Multi-OS Architecture**: Built around a clean Strategy Pattern (`Platform` trait) that isolates OS-specific behavior, allowing Linux and Windows support to be maintained without altering core extraction logic.

---

## Installation & Building

### Prerequisites

* Rust 1.80 or later (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
* macOS 12 or later (Apple Silicon M1/M2/M3/M4 or Intel x86_64)

### Building from Source

```bash
git clone git@github.com:mehdilight/kunai.git
cd kunai
cargo build --release
```

The compiled binary will be placed at `./target/release/kunai`.

To install it into your Cargo bin directory (accessible from any terminal session):

```bash
cargo install --path .
```

---

## Command-Line Usage

### 1. Extract a PKG to a Specific Directory

```bash
kunai Game.pkg /path/to/destination
```

If no destination is specified, Kunai extracts into a subfolder next to the source `.pkg` file.

### 2. Install Directly into shadPS4

```bash
kunai -i Game.pkg
```

Kunai inspects the package header and `param.sfo`, determines whether it is a base game, update patch, or DLC, and places it directly into your `~/Library/Application Support/shadPS4` directory.

### 3. Detect Package Type

Matches shadPS4Plus CLI exit codes:

```bash
kunai --check-type Game.pkg
echo $?
```

* Exit code `101`: Base Game
* Exit code `102`: Game Update / Patch
* Exit code `103`: DLC / Add-on Content

### 4. Display Package Metadata

```bash
kunai --info Game.pkg
```

Example output:

```text
=== Package Information ===
Title ID          : CUSA00001
Package Type      : Base Game
Content ID        : EP0001-CUSA00001_00-GAME000000000001
Package Size      : 25418291200 bytes
Entries Count     : 18

=== param.sfo Details ===
Game Title        : Bloodborne
Category          : gd
App Version       : 01.00
Version           : 01.00
```

### 5. Batch Process an Entire Directory

```bash
kunai --batch /path/to/pkg/folder
```

### 6. Reveal in Finder and Notify upon Completion

```bash
kunai -i -r --notify Game.pkg
```

---

## CLI Reference

```text
Usage: kunai [OPTIONS] [PKG_PATH] [OUTPUT_DIR]

Arguments:
  [PKG_PATH]    Path to the .pkg file or directory (when using --batch)
  [OUTPUT_DIR]  Destination directory for extracted contents

Options:
      --check-type             Detect and return PKG type exit code (101=game, 102=patch, 103=dlc)
  -i, --install                Install directly into shadPS4 directory
  -b, --batch                  Batch process all .pkg files in a directory
      --info                   Print detailed package metadata without extracting
  -r, --reveal                 Reveal output in Finder after extraction
      --notify                 Send desktop notification when extraction completes
      --games-dir <GAMES_DIR>  Override custom games directory
      --addons-dir <ADDONS_DIR> Override custom addons directory
  -h, --help                   Print help
  -V, --version                Print version
```

---

## Project Structure

```text
kunai/
├── Cargo.toml
├── src/
│   ├── main.rs                  # CLI entry point and progress bar driver
│   ├── lib.rs                   # Library root
│   ├── cli.rs                   # Command-line interface definitions
│   ├── core/                    # OS-agnostic core algorithms
│   │   ├── pkg.rs               # PKG container reader and type detector
│   │   ├── psf.rs               # PSF (param.sfo) format parser
│   │   ├── pfs.rs               # PFS/PFSC filesystem and Inode/Dirent tree
│   │   ├── entry_types.rs       # Canonical sce_sys file ID mappings
│   │   └── crypto/              # Cryptographic subsystem
│   │       ├── keys.rs          # Embedded RSA keysets
│   │       ├── rsa.rs           # RSA-2048 PKCS#1 v1.5 decryption
│   │       ├── aes.rs           # AES-128-CBC decryption
│   │       ├── xts.rs           # AES-128-XTS sector decryption
│   │       └── kdf.rs           # HMAC-SHA256 and SHA-256 derivation
│   └── platform/                # Platform Strategy Pattern
│       ├── mod.rs               # Platform trait and current_platform() factory
│       ├── macos.rs             # macOS implementation (Application Support, Finder, notifications)
│       ├── linux.rs             # Linux implementation (XDG specification)
│       └── windows.rs           # Windows implementation (APPDATA)
```

### Adding New Platforms

The `Platform` trait abstracts all OS-dependent paths and system tools:

```rust
pub trait Platform: Send + Sync {
    fn name(&self) -> &'static str;
    fn os_type(&self) -> OsType;
    fn default_shadps4_dir(&self) -> Option<PathBuf>;
    fn default_games_dir(&self) -> Option<PathBuf>;
    fn default_addons_dir(&self) -> Option<PathBuf>;
    fn reveal_in_file_manager(&self, path: &Path) -> Result<()>;
    fn send_notification(&self, title: &str, message: &str) -> Result<()>;
}
```

New platforms (such as FreeBSD) can be added simply by implementing this trait in `src/platform/` and registering the target in `src/platform/mod.rs`.

---

## License

Dual-licensed under either:

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
