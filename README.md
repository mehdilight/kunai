# 玉手箱 (Tamatebako) 📦✨

[![macOS](https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon%20%26%20Intel)-black?logo=apple)](https://apple.com)
[![Rust](https://img.shields.io/badge/Language-Rust%202021-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue)](LICENSE)

**Tamatebako (玉手箱)** — named after the legendary sealed treasure box of Japanese folklore — is a high-performance, native **macOS PlayStation 4 PKG extractor and shadPS4 installer** written in pure Rust.

It implements the cryptographic unpacking and PFS file extraction algorithm used in [shadPS4](https://github.com/shadps4-emu/shadPS4) and [shadPS4Plus](https://github.com/AzaharPlus/shadPS4Plus), natively compiled for Apple Silicon with zero external C/C++ dependencies.

---

## 🌟 Highlights

* **🍎 Apple Silicon Native**: Optimized for ARM64 (`aarch64-apple-darwin`) utilizing hardware-accelerated AES and SHA instructions.
* **🔑 Full Cryptographic Parity**:
  * Decrypts fake PKG (FPKG) entry keys via RSA-2048 PKCS#1 v1.5 (`dk3` and `ekpfs`).
  * AES-128-CBC decryption for `sce_sys` license and NP files (`license.dat`, `license.info`, `nptitle.dat`, `npbind.dat`).
  * AES-128-XTS sector-by-sector decryption for the PFS outer image and inner files.
  * Inode and Dirent flat-path directory tree reconstruction with zlib decompression.
* **🎮 Smart shadPS4 Integration**:
  * Resolves macOS user directories automatically (`~/Library/Application Support/shadPS4`).
  * Reads shadPS4's `config.toml` to honor custom game and addon paths.
  * Automatically routes packages to their correct destination:
    * **Base Games** (`101`) ➔ `games/<TITLE_ID>/`
    * **Game Updates / Patches** (`102`) ➔ `games/<TITLE_ID>-patch/`
    * **DLCs / Add-ons** (`103`) ➔ `user/addcont/<TITLE_ID>/<ENTITLEMENT_LABEL>/`
* **🖥️ macOS System Enhancements**:
  * `--reveal` (`-r`): Automatically opens the destination directory in Finder.
  * `--notify`: Sends macOS system banners and chime sounds upon completion.
* **🧩 Multi-OS Extensible Architecture**: Designed using the **Strategy Pattern** (`Platform` trait). While focused on macOS, implementations for Linux (XDG) and Windows are readily pluggable.

---

## 🚀 Installation & Building

### Prerequisites
* Rust 1.80+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
* macOS 12+ (Apple Silicon M1/M2/M3/M4 or Intel)

### Build from source
```bash
git clone git@github.com:mehdilight/tamatebako.git
cd tamatebako
cargo build --release
```
The optimized binary will be created at `./target/release/tamatebako`.

You can install it globally to your path via:
```bash
cargo install --path .
```

---

## 📖 Usage

### 1. Extract a PKG to a directory
```bash
tamatebako Game.pkg /path/to/output
```
*If no output directory is specified, it extracts next to the PKG file.*

### 2. Install directly into shadPS4 on macOS
```bash
tamatebako -i Game.pkg
```
Tamatebako inspects the `param.sfo` and flags, determines whether it is a base game, update, or DLC, and places it directly into your `~/Library/Application Support/shadPS4` directories!

### 3. Check PKG Type (Matches shadPS4Plus CLI exit codes)
```bash
tamatebako --check-type Game.pkg
echo $?
```
* Exit code `101`: Base Game
* Exit code `102`: Game Update / Patch
* Exit code `103`: DLC / Add-on Content

### 4. Inspect Package Information
```bash
tamatebako --info Game.pkg
```
Output:
```
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

### 5. Batch Extract an Entire Directory
```bash
tamatebako --batch /path/to/folder/with/pkgs
```

### 6. Reveal in Finder and Notify upon Completion
```bash
tamatebako -i -r --notify Game.pkg
```

---

## 📐 Architecture & Design Pattern

```
tamatebako/
├── Cargo.toml
├── src/
│   ├── main.rs                  # CLI driver and progress bar rendering
│   ├── lib.rs                   # Library crate entry
│   ├── cli.rs                   # Clap CLI definition
│   ├── core/                    # OS-agnostic extraction algorithms
│   │   ├── pkg.rs               # PKG container reader and type detector
│   │   ├── psf.rs               # PSF (param.sfo) parser
│   │   ├── pfs.rs               # PFS/PFSC filesystem and Inode/Dirent tree
│   │   ├── entry_types.rs       # Canonical sce_sys file ID mappings
│   │   └── crypto/              # Cryptographic subsystem
│   │       ├── keys.rs          # Embedded RSA keysets
│   │       ├── rsa.rs           # RSA-2048 PKCS#1 v1.5 decryption
│   │       ├── aes.rs           # AES-128-CBC decryption
│   │       ├── xts.rs           # AES-128-XTS sector decryption
│   │       └── kdf.rs           # HMAC-SHA256 & SHA-256 derivation
│   └── platform/                # Platform Strategy Pattern
│       ├── mod.rs               # Platform trait and factory
│       ├── macos.rs             # macOS implementation (Application Support, Finder, notifications)
│       ├── linux.rs             # Linux implementation (XDG specification)
│       └── windows.rs           # Windows implementation (APPDATA)
```

### Adding New Operating Systems
To add another platform (e.g. FreeBSD):
1. Implement the `Platform` trait in `src/platform/freebsd.rs`.
2. Register the target in `src/platform/mod.rs` inside `current_platform()`.
All core cryptographic and unpacking routines will work out of the box.

---

## 📜 License

Licensed under either of:
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
