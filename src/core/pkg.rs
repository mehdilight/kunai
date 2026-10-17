//! PlayStation 4 PKG container parser and extractor.

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use super::crypto::{
    aes_cbc_decrypt, aes_cbc_decrypt_vec, decrypt_dk3, decrypt_ekpfs, decrypt_pfs, derive_iv_key,
    derive_pfs_keys,
};
use super::entry_types::get_entry_name_by_type;
use super::pfs::{
    decompress_pfsc_block, find_pfsc_offset, Dirent, Inode, PfsFsEntry, PfscHeader,
    PFS_CURRENT_DIR, PFS_DIR, PFS_FILE,
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
    pub fn extract<P: AsRef<Path>, F: Fn(usize, usize, &str)>(
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

        // Read iNodes and Dirents from PFSC blocks
        let mut inode_buf = Vec::new();
        let mut fs_table = Vec::new();
        let mut extract_paths: HashMap<u32, PathBuf> = HashMap::new();
        let mut current_dir = destination_dir.clone();

        let mut ndinode = 0u32;
        let mut ndinode_counter = 0usize;
        let mut dinode_reached = false;
        let mut uroot_reached = false;
        let mut decompressed_data = vec![0u8; 0x10000];

        for i in 0..num_blocks {
            let sector_offset = sector_map[i] as usize;
            let sector_size = (sector_map[i + 1] - sector_map[i]) as usize;

            if sector_offset + sector_size > pfsc_data.len() {
                continue;
            }

            let comp_slice = &pfsc_data[sector_offset..sector_offset + sector_size];

            if sector_size == 0x10000 {
                decompressed_data.copy_from_slice(comp_slice);
            } else if sector_size < 0x10000 {
                let _ = decompress_pfsc_block(comp_slice, &mut decompressed_data);
            }

            if i == 0 {
                ndinode = LittleEndian::read_u32(&decompressed_data[0x30..0x34]);
            }

            let mut occupied_blocks = (ndinode as usize * 0xA8) / 0x10000;
            if (ndinode as usize * 0xA8) % 0x10000 != 0 {
                occupied_blocks += 1;
            }

            if i >= 1 && i <= occupied_blocks {
                let mut p = 0;
                while p < 0x10000 {
                    let node = Inode::parse(&decompressed_data[p..]);
                    if node.mode == 0 {
                        break;
                    }
                    inode_buf.push(node);
                    p += 0xA8;
                }
            }

            // Check for flat_path_table / uroot
            if decompressed_data.len() >= 0x1F {
                let flat_path_table = String::from_utf8_lossy(&decompressed_data[0x10..0x1F]);
                if flat_path_table.starts_with("flat_path_table") {
                    uroot_reached = true;
                }
            }

            if uroot_reached {
                let mut offset = 0;
                while offset < 0x10000 {
                    if let Some(dirent) = Dirent::parse(&decompressed_data[offset..]) {
                        let ent_size = dirent.entsize;
                        if dirent.ino != 0 {
                            ndinode_counter += 1;
                        } else {
                            let parent_path = destination_dir
                                .parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(|| destination_dir.clone());
                            let tid = self.title_id();

                            if parent_path.file_name().and_then(|n| n.to_str()) != Some(tid)
                                && !destination_dir.to_string_lossy().ends_with("-patch")
                            {
                                extract_paths.insert(ndinode_counter as u32, parent_path.join(tid));
                            } else {
                                extract_paths.insert(ndinode_counter as u32, destination_dir.clone());
                            }
                            uroot_reached = false;
                            break;
                        }
                        if ent_size == 0 {
                            break;
                        }
                        offset += ent_size;
                    } else {
                        break;
                    }
                }
            }

            if decompressed_data.len() >= 0x2A {
                let dot = decompressed_data[0x10];
                let dotdot = &decompressed_data[0x28..0x2A];
                if dot == b'.' && dotdot == b".." {
                    dinode_reached = true;
                }
            }

            if dinode_reached {
                let mut j = 0;
                let mut end_reached = false;
                while j < 0x10000 {
                    if let Some(dirent) = Dirent::parse(&decompressed_data[j..]) {
                        if dirent.ino == 0 {
                            break;
                        }

                        let ent_size = dirent.entsize;
                        let fs_entry = PfsFsEntry {
                            name: dirent.name.clone(),
                            inode: dirent.ino as u32,
                            entry_type: dirent.dirent_type,
                        };

                        if fs_entry.entry_type == PFS_CURRENT_DIR {
                            if let Some(path) = extract_paths.get(&fs_entry.inode) {
                                current_dir = path.clone();
                            }
                        }

                        let target_path = current_dir.join(&fs_entry.name);
                        extract_paths.insert(fs_entry.inode, target_path.clone());

                        if fs_entry.entry_type == PFS_FILE || fs_entry.entry_type == PFS_DIR {
                            if fs_entry.entry_type == PFS_DIR {
                                fs::create_dir_all(&target_path)?;
                            }
                            ndinode_counter += 1;
                            if ndinode_counter + 1 == ndinode as usize {
                                end_reached = true;
                            }
                        }

                        fs_table.push(fs_entry);

                        if ent_size == 0 {
                            break;
                        }
                        j += ent_size;
                    } else {
                        break;
                    }
                }

                if end_reached {
                    break;
                }
            }
        }

        // Step 3: Extract files from PFS image sectors
        let total_files = fs_table.iter().filter(|e| e.entry_type == PFS_FILE).count();
        let mut current_file_idx = 0;

        for entry in &fs_table {
            if entry.entry_type != PFS_FILE {
                continue;
            }

            current_file_idx += 1;
            progress_cb(current_file_idx, total_files, &entry.name);

            let inode_idx = entry.inode as usize;
            if inode_idx >= inode_buf.len() {
                continue;
            }

            let inode = &inode_buf[inode_idx];
            let sector_loc = inode.loc as usize;
            let nblocks = inode.blocks as usize;
            let file_size = inode.size as usize;

            let file_path = match extract_paths.get(&entry.inode) {
                Some(p) => p,
                None => continue,
            };

            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut out_file = File::create(file_path)?;
            let mut size_decompressed = 0usize;

            const BUFFER_SIZE: usize = 0x11000;
            let mut pfsc_buffer = vec![0u8; BUFFER_SIZE];
            let mut decrypted_buffer = vec![0u8; BUFFER_SIZE];
            let mut decomp_block = vec![0u8; 0x10000];

            for j in 0..nblocks {
                let sector_idx = sector_loc + j;
                if sector_idx + 1 >= sector_map.len() {
                    break;
                }

                let sec_offset = sector_map[sector_idx];
                let sec_size = (sector_map[sector_idx + 1] - sec_offset) as usize;

                let abs_offset = self.header.pfs_image_offset + (pfsc_offset as u64) + sec_offset;
                let sector_mask = (sec_offset + pfsc_offset as u64) & !0xFFF;
                let prev_data = ((sec_offset + pfsc_offset as u64) - sector_mask) as usize;
                let aligned_file_offset = abs_offset - prev_data as u64;
                let current_sector = sector_mask / 0x1000;

                self.file.seek(SeekFrom::Start(aligned_file_offset))?;
                let bytes_read = self.file.read(&mut pfsc_buffer)?;
                if bytes_read < BUFFER_SIZE {
                    pfsc_buffer[bytes_read..].fill(0);
                }

                decrypt_pfs(&data_key, &tweak_key, &pfsc_buffer, &mut decrypted_buffer, current_sector);

                let comp_slice = &decrypted_buffer[prev_data..prev_data + sec_size];

                if sec_size == 0x10000 {
                    decomp_block.copy_from_slice(comp_slice);
                } else if sec_size < 0x10000 {
                    let _ = decompress_pfsc_block(comp_slice, &mut decomp_block);
                }

                size_decompressed += 0x10000;

                if j < nblocks - 1 {
                    out_file.write_all(&decomp_block)?;
                } else {
                    // Trim zero padding on the last block
                    let excess = size_decompressed.saturating_sub(file_size);
                    let write_size = decomp_block.len().saturating_sub(excess);
                    out_file.write_all(&decomp_block[..write_size])?;
                }
            }
        }

        Ok(destination_dir)
    }
}
