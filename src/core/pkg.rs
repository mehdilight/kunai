//! PlayStation 4 PKG container parser and extractor.

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::crypto::{
    aes_cbc_decrypt, aes_cbc_decrypt_vec, decrypt_dk3, decrypt_ekpfs, decrypt_pfs, derive_iv_key,
    derive_pfs_keys,
};
use rayon::prelude::*;
use super::entry_types::get_entry_name_by_type;
use super::pfs::{
    enable_fast_io, find_pfsc_offset, parse_pfs_tree, process_pfs_block, PfscHeader,
};
use super::psf::Psf;

pub const PKG_MAGIC: u32 = 0x7F434E54; // "\x7FCNT"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgType {
    BaseGame = 101,
    Patch = 102,
    Dlc = 103,
}

impl PkgType {
    pub fn description(&self) -> &'static str {
        match self {
            PkgType::BaseGame => "Base Game",
            PkgType::Patch => "Game Update / Patch",
            PkgType::Dlc => "DLC / Add-on Content",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PkgHeader {
    pub magic: u32,
    pub pkg_type: u32,
    pub pkg_file_count: u32,
    pub pkg_table_entry_count: u32,
    pub pkg_table_entry_offset: u32,
    pub pkg_body_offset: u64,
    pub pkg_body_size: u64,
    pub pkg_content_offset: u64,
    pub pkg_content_size: u64,
    pub content_id: String,
    pub title_id: String,
    pub pkg_content_flags: u32,
    pub pfs_image_offset: u64,
    pub pfs_image_size: u64,
    pub pkg_size: u64,
    pub pfs_cache_size: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PkgEntry {
    pub id: u32,
    pub filename_offset: u32,
    pub flags1: u32,
    pub flags2: u32,
    pub offset: u32,
    pub size: u32,
    pub raw_header: [u8; 32],
}

pub struct Pkg {
    pub path: PathBuf,
    pub header: PkgHeader,
    pub entries: Vec<PkgEntry>,
    pub sfo: Option<Psf>,
    pub sfo_raw: Option<Vec<u8>>,
    file: File,
}

impl Pkg {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)
            .with_context(|| format!("Failed to open PKG file: {}", path.display()))?;

        let total_size = file.metadata()?.len();
        if total_size < 0x1000 {
            return Err(anyhow!("File is too small to be a valid PKG"));
        }

        let mut header_buf = [0u8; 0x1000];
        file.read_exact(&mut header_buf)?;

        let magic = BigEndian::read_u32(&header_buf[0..4]);
        if magic != PKG_MAGIC {
            return Err(anyhow!(
                "Invalid PKG magic: expected 0x{:08X}, got 0x{:08X}",
                PKG_MAGIC,
                magic
            ));
        }

        let pkg_type = BigEndian::read_u32(&header_buf[4..8]);
        let pkg_file_count = BigEndian::read_u32(&header_buf[12..16]);
        let pkg_table_entry_count = BigEndian::read_u32(&header_buf[16..20]);
        let pkg_table_entry_offset = BigEndian::read_u32(&header_buf[24..28]);
        let pkg_body_offset = BigEndian::read_u64(&header_buf[32..40]);
        let pkg_body_size = BigEndian::read_u64(&header_buf[40..48]);
        let pkg_content_offset = BigEndian::read_u64(&header_buf[48..56]);
        let pkg_content_size = BigEndian::read_u64(&header_buf[56..64]);

        // Content ID at 0x40 (36 bytes)
        let cid_bytes = &header_buf[0x40..0x40 + 36];
        let cid_end = cid_bytes.iter().position(|&b| b == 0).unwrap_or(36);
        let content_id = String::from_utf8_lossy(&cid_bytes[..cid_end]).to_string();

        // Title ID at 0x47 (9 characters)
        let title_id = if header_buf.len() >= 0x47 + 9 {
            String::from_utf8_lossy(&header_buf[0x47..0x47 + 9]).to_string()
        } else {
            String::new()
        };

        let pkg_content_flags = BigEndian::read_u32(&header_buf[0x78..0x7C]);
        let pfs_image_offset = BigEndian::read_u64(&header_buf[0x410..0x418]);
        let pfs_image_size = BigEndian::read_u64(&header_buf[0x418..0x420]);
        let pkg_size = BigEndian::read_u64(&header_buf[0x430..0x438]);
        let pfs_cache_size = BigEndian::read_u32(&header_buf[0x43C..0x440]);

        let header = PkgHeader {
            magic,
            pkg_type,
            pkg_file_count,
            pkg_table_entry_count,
            pkg_table_entry_offset,
            pkg_body_offset,
            pkg_body_size,
            pkg_content_offset,
            pkg_content_size,
            content_id,
            title_id,
            pkg_content_flags,
            pfs_image_offset,
            pfs_image_size,
            pkg_size,
            pfs_cache_size,
        };

        // Read entries table
        file.seek(SeekFrom::Start(header.pkg_table_entry_offset as u64))?;
        let mut entries = Vec::with_capacity(header.pkg_table_entry_count as usize);

        let mut sfo_offset: Option<u32> = None;
        let mut sfo_size: Option<u32> = None;

        for _ in 0..header.pkg_table_entry_count {
            let mut entry_buf = [0u8; 32];
            file.read_exact(&mut entry_buf)?;

            let id = BigEndian::read_u32(&entry_buf[0..4]);
            let filename_offset = BigEndian::read_u32(&entry_buf[4..8]);
            let flags1 = BigEndian::read_u32(&entry_buf[8..12]);
            let flags2 = BigEndian::read_u32(&entry_buf[12..16]);
            let offset = BigEndian::read_u32(&entry_buf[16..20]);
            let size = BigEndian::read_u32(&entry_buf[20..24]);

            if id == 0x1000 {
                sfo_offset = Some(offset);
                sfo_size = Some(size);
            }

            entries.push(PkgEntry {
                id,
                filename_offset,
                flags1,
                flags2,
                offset,
                size,
                raw_header: entry_buf,
            });
        }

        // Read and parse param.sfo if present
        let mut sfo = None;
        let mut sfo_raw = None;
        if let (Some(offset), Some(size)) = (sfo_offset, sfo_size) {
            file.seek(SeekFrom::Start(offset as u64))?;
            let mut sfo_buf = vec![0u8; size as usize];
            file.read_exact(&mut sfo_buf)?;
            if let Ok(parsed) = Psf::parse(&sfo_buf) {
                sfo = Some(parsed);
            }
            sfo_raw = Some(sfo_buf);
        }

        Ok(Self {
            path,
            header,
            entries,
            sfo,
            sfo_raw,
            file,
        })
    }

    pub fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub fn file_seek(&mut self, pos: u64) -> Result<()> {
        self.file.seek(SeekFrom::Start(pos))?;
        Ok(())
    }

    pub fn file_read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.file.read_exact(buf)?;
        Ok(())
    }

    /// Determines the type of PKG (Base Game, Patch, or DLC).
    pub fn detect_type(&self) -> PkgType {
        // Check content flags for patch markers
        let flags = self.header.pkg_content_flags;
        let is_patch = (flags & 0x100000 != 0)     // FIRST_PATCH
            || (flags & 0x40000000 != 0)           // SUBSEQUENT_PATCH
            || (flags & 0x41000000 == 0x41000000)  // DELTA_PATCH
            || (flags & 0x60000000 == 0x60000000); // CUMULATIVE_PATCH

        if is_patch {
            return PkgType::Patch;
        }

        // Check param.sfo category
        if let Some(sfo) = &self.sfo {
            if let Some(category) = sfo.category() {
                if category == "ac" {
                    return PkgType::Dlc;
                }
            }
        }

        PkgType::BaseGame
    }

    pub fn title_id(&self) -> &str {
        if let Some(sfo) = &self.sfo {
            if let Some(tid) = sfo.title_id() {
                if !tid.is_empty() {
                    return tid;
                }
            }
        }
        &self.header.title_id
    }

    pub fn entitlement_label(&self) -> Option<String> {
        let content_id = if let Some(sfo) = &self.sfo {
            sfo.content_id().unwrap_or(&self.header.content_id)
        } else {
            &self.header.content_id
        };

        let parts: Vec<&str> = content_id.split('-').collect();
        if parts.len() >= 3 {
            Some(parts[2].to_string())
        } else {
            None
        }
    }

    /// Extracts the entire PKG contents (sce_sys entries, decrypted NP entries, and PFS image).
    pub fn extract<P: AsRef<Path>, F: Fn(usize, usize, &str) + Send>(
        &mut self,
        destination_dir: P,
        progress_cb: F,
    ) -> Result<PathBuf> {
        let destination_dir = destination_dir.as_ref().to_path_buf();
        fs::create_dir_all(&destination_dir)?;

        let mut dk3: Option<[u8; 32]> = None;
        let mut ekpfs: Option<[u8; 32]> = None;

        // Step 1: Extract sce_sys files & find decryption keys
        let sce_sys_dir = destination_dir.join("sce_sys");
        fs::create_dir_all(&sce_sys_dir)?;

        for entry in &self.entries {
            self.file.seek(SeekFrom::Start(entry.offset as u64))?;

            if entry.id == 0x10 {
                // ENTRY_KEYS: Read seed_digest, 7 digests, 7 keys
                let mut seed_digest = [0u8; 32];
                self.file.read_exact(&mut seed_digest)?;

                let mut digests = [[0u8; 32]; 7];
                for d in digests.iter_mut() {
                    self.file.read_exact(d)?;
                }

                let mut keys = [[0u8; 256]; 7];
                for k in keys.iter_mut() {
                    self.file.read_exact(k)?;
                }

                // Decrypt DK3 from key1[3]
                if let Ok(derived_dk3) = decrypt_dk3(&keys[3]) {
                    dk3 = Some(derived_dk3);
                }
            } else if entry.id == 0x20 {
                // IMAGE_KEY
                let mut imgkeydata = [0u8; 256];
                self.file.read_exact(&mut imgkeydata)?;

                if let Some(dk3_val) = &dk3 {
                    let iv_key = derive_iv_key(&entry.raw_header, dk3_val);
                    let mut img_key = [0u8; 256];
                    if aes_cbc_decrypt(&iv_key, &imgkeydata, &mut img_key).is_ok() {
                        if let Ok(derived_ekpfs) = decrypt_ekpfs(&img_key) {
                            ekpfs = Some(derived_ekpfs);
                        }
                    }
                }
            }

            // Extract entry file into sce_sys
            self.file.seek(SeekFrom::Start(entry.offset as u64))?;
            let mut data = vec![0u8; entry.size as usize];
            self.file.read_exact(&mut data)?;

            let filename = get_entry_name_by_type(entry.id)
                .map(|s| s.to_string())
                .unwrap_or_else(|| entry.id.to_string());

            let target_file_path = sce_sys_dir.join(&filename);
            if let Some(parent) = target_file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Decrypt NP entries (0x400 - 0x403)
            if [0x400, 0x401, 0x402, 0x403].contains(&entry.id) {
                if let Some(dk3_val) = &dk3 {
                    let iv_key = derive_iv_key(&entry.raw_header, dk3_val);
                    if let Ok(decrypted) = aes_cbc_decrypt_vec(&iv_key, &data) {
                        data = decrypted;
                    }
                }
            }

            fs::write(&target_file_path, &data)?;
        }

        // Step 2: Decrypt and extract PFS filesystem
        let ekpfs_key = ekpfs.ok_or_else(|| anyhow!("Could not decrypt EKPFS key; PKG may be retail or unsupported"))?;

        // Read seed at pfs_image_offset + 0x370
        self.file.seek(SeekFrom::Start(self.header.pfs_image_offset + 0x370))?;
        let mut seed = [0u8; 16];
        self.file.read_exact(&mut seed)?;

        let (data_key, tweak_key) = derive_pfs_keys(&ekpfs_key, &seed);

        let pfs_cache_len = (self.header.pfs_cache_size as usize) * 2;
        if pfs_cache_len == 0 {
            return Ok(destination_dir);
        }

        // Read and decrypt PFS header image
        self.file.seek(SeekFrom::Start(self.header.pfs_image_offset))?;
        let mut pfs_encrypted = vec![0u8; pfs_cache_len];
        self.file.read_exact(&mut pfs_encrypted)?;

        let mut pfs_decrypted = vec![0u8; pfs_cache_len];
        decrypt_pfs(&data_key, &tweak_key, &pfs_encrypted, &mut pfs_decrypted, 0);

        let pfsc_offset = find_pfsc_offset(&pfs_decrypted)
            .ok_or_else(|| anyhow!("Could not find PFSC magic in decrypted PFS image"))?;

        let pfsc_data = &pfs_decrypted[pfsc_offset..];
        let pfsc_hdr = PfscHeader::parse(pfsc_data)?;

        let num_blocks = (pfsc_hdr.data_length / pfsc_hdr.block_sz2) as usize;
        let mut sector_map = Vec::with_capacity(num_blocks + 1);

        for i in 0..=num_blocks {
            let offset = (pfsc_hdr.block_offsets as usize) + i * 8;
            if offset + 8 <= pfsc_data.len() {
                sector_map.push(LittleEndian::read_u64(&pfsc_data[offset..offset + 8]));
            }
        }

        if sector_map.len() <= num_blocks {
            return Err(anyhow!("Invalid sector map in PFSC header"));
        }

        // Parse PFS directory tree hierarchically
        let pfs_entries = parse_pfs_tree(
            pfsc_data,
            &self.file,
            self.header.pfs_image_offset,
            pfsc_offset,
            &sector_map,
            &data_key,
            &tweak_key,
        )?;

        // Step 3: Extract files from PFS image sectors
        enable_fast_io(&self.file);

        // Pre-create all directory nodes
        for entry in &pfs_entries {
            if entry.is_dir {
                let dir_path = destination_dir.join(&entry.relative_path);
                let _ = fs::create_dir_all(&dir_path);
            }
        }

        struct FileToExtract {
            name: String,
            file_path: PathBuf,
            sector_loc: usize,
            nblocks: usize,
            file_size: usize,
        }

        let files_to_extract: Vec<_> = pfs_entries
            .into_iter()
            .filter(|e| !e.is_dir)
            .map(|e| {
                let file_path = destination_dir.join(&e.relative_path);
                FileToExtract {
                    name: e.name,
                    file_path,
                    sector_loc: e.loc as usize,
                    nblocks: e.blocks as usize,
                    file_size: e.size as usize,
                }
            })
            .collect();

        let total_files = files_to_extract.len();
        let progress_counter = std::sync::atomic::AtomicUsize::new(0);
        let progress_mutex = std::sync::Mutex::new(progress_cb);

        let report_done = |file_name: &str| {
            let current = progress_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if let Ok(cb) = progress_mutex.lock() {
                cb(current, total_files, file_name);
            }
        };

        let extract_single_file = |item: &FileToExtract| -> Result<()> {
            if let Some(parent) = item.file_path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let out_file = File::create(&item.file_path)?;
            if item.nblocks == 0 || item.file_size == 0 {
                return Ok(());
            }

            let mut writer = std::io::BufWriter::with_capacity(1024 * 1024, out_file);

            if item.nblocks == 1 {
                let mut block = [0u8; 0x10000];
                process_pfs_block(
                    &self.file,
                    self.header.pfs_image_offset,
                    pfsc_offset,
                    &sector_map,
                    &data_key,
                    &tweak_key,
                    item.sector_loc,
                    &mut block,
                )?;
                let write_size = item.file_size.min(0x10000);
                writer.write_all(&block[..write_size])?;
                writer.flush()?;
                return Ok(());
            }

            // Multi-block file: process in parallel chunks across CPU cores
            const CHUNK_SIZE: usize = 64; // 64 blocks = 4 MB per chunk
            let mut chunk_blocks: Vec<[u8; 0x10000]> = vec![[0u8; 0x10000]; CHUNK_SIZE];

            for chunk_start in (0..item.nblocks).step_by(CHUNK_SIZE) {
                let chunk_len = (item.nblocks - chunk_start).min(CHUNK_SIZE);
                let active_slice = &mut chunk_blocks[..chunk_len];

                active_slice
                    .par_iter_mut()
                    .enumerate()
                    .try_for_each(|(idx, block)| {
                        let j = chunk_start + idx;
                        process_pfs_block(
                            &self.file,
                            self.header.pfs_image_offset,
                            pfsc_offset,
                            &sector_map,
                            &data_key,
                            &tweak_key,
                            item.sector_loc + j,
                            block,
                        )
                    })?;

                for (idx, block) in active_slice.iter().enumerate() {
                    let j = chunk_start + idx;
                    if j < item.nblocks - 1 {
                        writer.write_all(block)?;
                    } else {
                        let excess = (item.nblocks * 0x10000).saturating_sub(item.file_size);
                        let write_size = 0x10000usize.saturating_sub(excess);
                        writer.write_all(&block[..write_size])?;
                    }
                }
            }

            writer.flush()?;
            Ok(())
        };

        // Partition into small files (<= 4 blocks, <= 256 KB) and large files (> 4 blocks)
        let (small_files, large_files): (Vec<_>, Vec<_>) = files_to_extract
            .into_iter()
            .partition(|item| item.nblocks <= 4);

        // Extract small files in parallel across CPU cores
        small_files.par_iter().try_for_each(|item| -> Result<()> {
            extract_single_file(item)?;
            report_done(&item.name);
            Ok(())
        })?;

        // Extract large files (each large file uses all CPU cores for block decompression)
        for item in &large_files {
            extract_single_file(item)?;
            report_done(&item.name);
        }

        Ok(destination_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sample_pkg() {
        let pkg_path = Path::new("/Users/mehdi/projects/fun/rsx4/IV0000-BREW00777_00-RSX4EMULATOR0001.pkg");
        if !pkg_path.exists() {
            return;
        }

        let mut pkg = Pkg::open(pkg_path).expect("Failed to open test PKG");
        assert_eq!(pkg.detect_type(), PkgType::BaseGame);
        assert_eq!(pkg.title_id(), "BREW00777");

        let out_dir = std::env::temp_dir().join(format!("kunai_test_{}", std::process::id()));
        if out_dir.exists() {
            let _ = fs::remove_dir_all(&out_dir);
        }

        let reported_files = std::sync::Mutex::new(Vec::new());
        let dest = pkg
            .extract(&out_dir, |_cur, _tot, name| {
                if let Ok(mut list) = reported_files.lock() {
                    list.push(name.to_string());
                }
            })
            .expect("Extraction failed");

        assert!(dest.exists());
        assert!(dest.join("sce_sys/param.sfo").exists());
        assert!(dest.join("eboot.bin").exists());

        let eboot_meta = fs::metadata(dest.join("eboot.bin")).expect("eboot.bin metadata");
        assert!(eboot_meta.len() > 0, "eboot.bin must not be empty");

        // Verify .prx module files extraction
        assert!(dest.join("sce_module/libc.prx").exists(), "libc.prx must be extracted into sce_module");
        let libc_meta = fs::metadata(dest.join("sce_module/libc.prx")).expect("libc.prx metadata");
        assert_eq!(libc_meta.len(), 52800, "libc.prx size mismatch");

        assert!(dest.join("sce_module/libSceFios2.prx").exists(), "libSceFios2.prx must be extracted into sce_module");
        let fios2_meta = fs::metadata(dest.join("sce_module/libSceFios2.prx")).expect("libSceFios2.prx metadata");
        assert_eq!(fios2_meta.len(), 52816, "libSceFios2.prx size mismatch");

        // Verify subfolder binaries
        assert!(dest.join("games/tetris.elf").exists(), "tetris.elf must be extracted into games");
        let tetris_meta = fs::metadata(dest.join("games/tetris.elf")).expect("tetris.elf metadata");
        assert_eq!(tetris_meta.len(), 548524, "tetris.elf size mismatch");

        assert!(dest.join("sce_sys/about/right.sprx").exists(), "right.sprx must be extracted into sce_sys/about");

        let _ = fs::remove_dir_all(&out_dir);
    }
}
