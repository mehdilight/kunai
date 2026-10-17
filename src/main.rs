//! Kunai (苦無) - PlayStation 4 PKG extractor for macOS.

use anyhow::{anyhow, Result};
use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kunai::cli::Cli;
use kunai::core::pkg::{Pkg, PkgType};
use kunai::platform::{current_platform, Platform};

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{} {}", "Error:".bold().red(), err);
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let platform = current_platform();

    let pkg_path = match &cli.pkg_path {
        Some(p) => p.clone(),
        None => {
            eprintln!(
                "{}\nRun {} for usage details.",
                "Kunai - No PKG file or directory specified.".yellow(),
                "kunai --help".bold().cyan()
            );
            return Ok(ExitCode::from(1));
        }
    };

    if cli.batch || pkg_path.is_dir() {
        return run_batch(&cli, &*platform, &pkg_path);
    }

    run_single(&cli, &*platform, &pkg_path)
}

fn run_single(cli: &Cli, platform: &dyn Platform, pkg_path: &Path) -> Result<ExitCode> {
    if !pkg_path.exists() {
        return Err(anyhow!("PKG file not found: {}", pkg_path.display()));
    }

    let mut pkg = Pkg::open(pkg_path)?;
    let pkg_type = pkg.detect_type();
    let title_id = pkg.title_id().to_string();
    let entitlement_label = pkg.entitlement_label();

    // Check-type flag (matches pkg_extractor exit codes 101, 102, 103)
    if cli.check_type {
        match pkg_type {
            PkgType::BaseGame => {
                println!("pkg is a base game");
                return Ok(ExitCode::from(101));
            }
            PkgType::Patch => {
                println!("pkg is a game update");
                return Ok(ExitCode::from(102));
            }
            PkgType::Dlc => {
                println!("pkg is a dlc");
                return Ok(ExitCode::from(103));
            }
        }
    }

    // Info flag
    if cli.info {
        print_pkg_info(&pkg, pkg_type);
        return Ok(ExitCode::SUCCESS);
    }

    // Determine target extraction directory
    let target_dir = if cli.install {
        let dir = platform.resolve_target_dir(
            pkg_type,
            &title_id,
            entitlement_label.as_deref(),
            cli.games_dir.as_deref(),
            cli.addons_dir.as_deref(),
        );
        println!(
            "Installing directly into shadPS4 directory: {}",
            dir.display().to_string().cyan()
        );
        dir
    } else if let Some(base_output) = &cli.output_dir {
        resolve_custom_output(base_output, pkg_type, &title_id, entitlement_label.as_deref())
    } else {
        // Default: extract next to the PKG file
        let parent = pkg_path.parent().unwrap_or_else(|| Path::new("."));
        resolve_custom_output(parent, pkg_type, &title_id, entitlement_label.as_deref())
    };

    println!(
        "[{}] [{}] -> {}",
        pkg_path.file_name().and_then(|n| n.to_str()).unwrap_or("PKG"),
        pkg_type.description().yellow(),
        target_dir.display().to_string().bold()
    );

    // Extraction progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")?
            .progress_chars("#>-"),
    );

    let destination = pkg.extract(&target_dir, |current, total, file_name| {
        pb.set_length(total as u64);
        pb.set_position(current as u64);
        pb.set_message(file_name.to_string());
    })?;

    pb.finish_with_message("Done!".green().to_string());

    println!(
        "Successfully extracted to: {}",
        destination.display().to_string().bold().green()
    );

    if cli.reveal {
        if let Err(e) = platform.reveal_in_file_manager(&destination) {
            eprintln!("Warning: Failed to reveal in Finder: {}", e);
        }
    }

    if cli.notify {
        let msg = format!("Extracted {} ({})", title_id, pkg_type.description());
        let _ = platform.send_notification("Kunai", &msg);
    }

    Ok(ExitCode::SUCCESS)
}

fn run_batch(cli: &Cli, platform: &dyn Platform, dir_path: &Path) -> Result<ExitCode> {
    println!(
        "Scanning directory for .pkg files: {}",
        dir_path.display()
    );

    let mut pkg_files = Vec::new();
    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("pkg") {
                    pkg_files.push(path);
                }
            }
        }
    }

    if pkg_files.is_empty() {
        println!("No .pkg files found in {}", dir_path.display());
        return Ok(ExitCode::SUCCESS);
    }

    println!("Found {} .pkg package(s) to process.\n", pkg_files.len());

    for (idx, pkg_file) in pkg_files.iter().enumerate() {
        println!(
            "[{}/{}] Processing: {}",
            idx + 1,
            pkg_files.len(),
            pkg_file.file_name().and_then(|n| n.to_str()).unwrap_or("")
        );

        if let Err(err) = run_single(cli, platform, pkg_file) {
            eprintln!("{} Failed to process {}: {}", "Error:".red(), pkg_file.display(), err);
        }
        println!();
    }

    if cli.notify {
        let _ = platform.send_notification(
            "Kunai",
            &format!("Batch extraction finished ({} packages)", pkg_files.len()),
        );
    }

    Ok(ExitCode::SUCCESS)
}

fn resolve_custom_output(
    base: &Path,
    pkg_type: PkgType,
    title_id: &str,
    entitlement_label: Option<&str>,
) -> PathBuf {
    match pkg_type {
        PkgType::BaseGame => base.join(title_id),
        PkgType::Patch => base.join(format!("{}-patch", title_id)),
        PkgType::Dlc => {
            let label = entitlement_label.unwrap_or("content");
            base.join(title_id).join(label)
        }
    }
}

fn print_pkg_info(pkg: &Pkg, pkg_type: PkgType) {
    println!("{}", "=== Package Information ===".bold().cyan());
    println!("{:<18}: {}", "Title ID", pkg.title_id().bold());
    println!("{:<18}: {}", "Package Type", pkg_type.description().yellow());
    println!("{:<18}: {}", "Content ID", pkg.header.content_id);
    println!("{:<18}: {} bytes", "Package Size", pkg.header.pkg_size);
    println!("{:<18}: {}", "Entries Count", pkg.entries.len());

    if let Some(sfo) = &pkg.sfo {
        println!("\n{}", "=== param.sfo Details ===".bold().cyan());
        if let Some(title) = sfo.title() {
            println!("{:<18}: {}", "Game Title", title.bold().green());
        }
        if let Some(cat) = sfo.category() {
            println!("{:<18}: {}", "Category", cat);
        }
        if let Some(app_ver) = sfo.app_ver() {
            println!("{:<18}: {}", "App Version", app_ver);
        }
        if let Some(ver) = sfo.version() {
            println!("{:<18}: {}", "Version", ver);
        }
    }
}
