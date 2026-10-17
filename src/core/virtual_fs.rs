//! In-memory virtual filesystem tree for exploring PS4 PKG contents without extracting.

use anyhow::{anyhow, Result};
use byteorder::{ByteOrder, LittleEndian};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use super::crypto::{
    aes_cbc_decrypt_vec, decrypt_dk3, decrypt_ekpfs, decrypt_pfs, derive_iv_key, derive_pfs_keys,
};
use super::entry_types::get_entry_name_by_type;
use super::pfs::{
    decompress_pfsc_block, find_pfsc_offset, Dirent, Inode, PfsFsEntry, PfscHeader,
    PFS_CURRENT_DIR, PFS_DIR, PFS_FILE,
};
use super::pkg::Pkg;

#[derive(Debug, Clone)]
pub struct FsNode {
    pub name: String,
    pub relative_path: String,
    pub parent_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub inode: Option<u32>,
    pub loc: Option<u32>,
    pub blocks: Option<u32>,
    pub is_sce_sys: bool,
    pub sce_sys_offset: Option<u32>,
    pub sce_sys_size: Option<u32>,
    pub sce_sys_id: Option<u32>,
    pub raw_header: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Default)]
pub struct VirtualFs {
    pub nodes: Vec<FsNode>,
    pub sector_map: Vec<u64>,
    pub pfsc_offset: usize,
    pub data_key: [u8; 16],
    pub tweak_key: [u8; 16],
    pub dk3: Option<[u8; 32]>,
    pub pfs_image_offset: u64,
}

impl VirtualFs {
    /// Builds the virtual filesystem tree for the given PKG file.
    pub fn build(pkg: &mut Pkg) -> Result<Self> {
        let mut dk3: Option<[u8; 32]> = None;
        let mut ekpfs: Option<[u8; 32]> = None;

        let mut nodes: Vec<FsNode> = Vec::new();

        // 1. Always create the virtual sce_sys directory
        nodes.push(FsNode {
            name: "sce_sys".to_string(),
            relative_path: "sce_sys".to_string(),
            parent_path: "".to_string(),
            is_dir: true,
            size: 0,
            inode: None,
            loc: None,
            blocks: None,
            is_sce_sys: true,
            sce_sys_offset: None,
            sce_sys_size: None,
            sce_sys_id: None,
            raw_header: None,
        });

        let entries = pkg.entries.clone();
        for entry in &entries {
            if entry.id == 0x10 {
                // ENTRY_KEYS
                pkg.file_seek(entry.offset as u64)?;
                let mut seed_digest = [0u8; 32];
                pkg.file_read_exact(&mut seed_digest)?;

                let mut digests = [[0u8; 32]; 7];
                for d in digests.iter_mut() {
                    pkg.file_read_exact(d)?;
                }

                let mut keys = [[0u8; 256]; 7];
                for k in keys.iter_mut() {
                    pkg.file_read_exact(k)?;
                }

                if let Ok(derived_dk3) = decrypt_dk3(&keys[3]) {
                    dk3 = Some(derived_dk3);
                }
            } else if entry.id == 0x20 {
                // IMAGE_KEY
                pkg.file_seek(entry.offset as u64)?;
                let mut imgkeydata = [0u8; 256];
                pkg.file_read_exact(&mut imgkeydata)?;

                if let Some(dk3_val) = &dk3 {
                    let iv_key = derive_iv_key(&entry.raw_header, dk3_val);
                    let mut img_key = [0u8; 256];
                    if super::crypto::aes_cbc_decrypt(&iv_key, &imgkeydata, &mut img_key).is_ok() {
                        if let Ok(derived_ekpfs) = decrypt_ekpfs(&img_key) {
                            ekpfs = Some(derived_ekpfs);
                        }
                    }
                }
            }

            let filename = get_entry_name_by_type(entry.id)
                .map(|s| s.to_string())
                .unwrap_or_else(|| entry.id.to_string());

            let rel_path = format!("sce_sys/{}", filename);
            let parent = "sce_sys".to_string();

            nodes.push(FsNode {
                name: filename,
                relative_path: rel_path,
                parent_path: parent,
                is_dir: false,
                size: entry.size as u64,
                inode: None,
                loc: None,
                blocks: None,
                is_sce_sys: true,
                sce_sys_offset: Some(entry.offset),
                sce_sys_size: Some(entry.size),
                sce_sys_id: Some(entry.id),
                raw_header: Some(entry.raw_header),
            });
        }

        // 3. Scan PFS image and parse inner directory structure
        let ekpfs_key = match ekpfs {
            Some(k) => k,
            None => {
                // Return what we have from sce_sys if retail or no key
                return Ok(Self {
                    nodes,
                    sector_map: Vec::new(),
                    pfsc_offset: 0,
                    data_key: [0u8; 16],
                    tweak_key: [0u8; 16],
                    dk3,
                    pfs_image_offset: pkg.header.pfs_image_offset,
                });
            }
        };

        pkg.file_seek(pkg.header.pfs_image_offset + 0x370)?;
        let mut seed = [0u8; 16];
        pkg.file_read_exact(&mut seed)?;

        let (data_key, tweak_key) = derive_pfs_keys(&ekpfs_key, &seed);

        let pfs_cache_len = (pkg.header.pfs_cache_size as usize) * 2;
        if pfs_cache_len == 0 {
            return Ok(Self {
                nodes,
                sector_map: Vec::new(),
                pfsc_offset: 0,
                data_key,
                tweak_key,
                dk3,
                pfs_image_offset: pkg.header.pfs_image_offset,
            });
        }

        pkg.file_seek(pkg.header.pfs_image_offset)?;
        let mut pfs_encrypted = vec![0u8; pfs_cache_len];
        pkg.file_read_exact(&mut pfs_encrypted)?;

        let mut pfs_decrypted = vec![0u8; pfs_cache_len];
        decrypt_pfs(&data_key, &tweak_key, &pfs_encrypted, &mut pfs_decrypted, 0);

        let pfsc_offset = match find_pfsc_offset(&pfs_decrypted) {
            Some(o) => o,
            None => {
                return Ok(Self {
                    nodes,
                    sector_map: Vec::new(),
                    pfsc_offset: 0,
                    data_key,
                    tweak_key,
                    dk3,
                    pfs_image_offset: pkg.header.pfs_image_offset,
                });
            }
        };

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

        // Parse Inodes and Dirents
        let mut inode_buf = Vec::new();
        let mut fs_table = Vec::new();
        let mut extract_paths: HashMap<u32, String> = HashMap::new();
        let mut current_dir = "".to_string();

        let mut ndinode = 0u32;
        let mut ndinode_counter = 0usize;
        let mut dinode_reached = false;
        let mut uroot_reached = false;

        let mut decompressed_data = vec![0u8; 0x10000];

        for i in 0..num_blocks {
            if i >= sector_map.len() || i + 1 >= sector_map.len() {
                break;
            }

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
                            extract_paths.insert(ndinode_counter as u32, "".to_string());
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

                        let target_path = if current_dir.is_empty() {
                            fs_entry.name.clone()
                        } else {
                            format!("{}/{}", current_dir, fs_entry.name)
                        };

                        extract_paths.insert(fs_entry.inode, target_path.clone());

                        if fs_entry.entry_type == PFS_FILE || fs_entry.entry_type == PFS_DIR {
                            let inode_idx = fs_entry.inode as usize;
                            let (size, loc, blocks) = if inode_idx < inode_buf.len() {
                                let node = &inode_buf[inode_idx];
                                (node.size as u64, Some(node.loc), Some(node.blocks))
                            } else {
                                (0, None, None)
                            };

                            let parent_p = if current_dir.is_empty() {
                                "".to_string()
                            } else {
                                current_dir.clone()
                            };

                            nodes.push(FsNode {
                                name: fs_entry.name.clone(),
                                relative_path: target_path.clone(),
                                parent_path: parent_p,
                                is_dir: fs_entry.entry_type == PFS_DIR,
                                size,
                                inode: Some(fs_entry.inode),
                                loc,
                                blocks,
                                is_sce_sys: false,
                                sce_sys_offset: None,
                                sce_sys_size: None,
                                sce_sys_id: None,
                                raw_header: None,
                            });

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

        Ok(Self {
            nodes,
            sector_map,
            pfsc_offset,
            data_key,
            tweak_key,
            dk3,
            pfs_image_offset: pkg.header.pfs_image_offset,
        })
    }

    /// Lists entries located directly inside `parent_path` (e.g. `""` for root, `"sce_sys"` for sce_sys).
    pub fn list_dir(&self, parent_path: &str) -> Vec<&FsNode> {
        self.nodes
            .iter()
            .filter(|n| n.parent_path == parent_path)
            .collect()
    }

    /// Finds a node by exact relative path.
    pub fn find_node(&self, path: &str) -> Option<&FsNode> {
        self.nodes.iter().find(|n| n.relative_path == path)
    }

    /// Reads and decrypts the entire content of a single file on-demand in memory.
    pub fn read_file_content(&self, pkg_file: &mut File, node: &FsNode) -> Result<Vec<u8>> {
        if node.is_dir {
            return Err(anyhow!("Cannot read contents of a directory"));
        }

        if node.is_sce_sys {
            let offset = node.sce_sys_offset.ok_or_else(|| anyhow!("Missing offset for sce_sys file"))?;
            let size = node.sce_sys_size.ok_or_else(|| anyhow!("Missing size for sce_sys file"))?;

            pkg_file.seek(SeekFrom::Start(offset as u64))?;
            let mut data = vec![0u8; size as usize];
            pkg_file.read_exact(&mut data)?;

            if let (Some(id), Some(dk3_val), Some(header)) = (node.sce_sys_id, &self.dk3, node.raw_header) {
                if [0x400, 0x401, 0x402, 0x403].contains(&id) {
                    let iv_key = derive_iv_key(&header, dk3_val);
                    if let Ok(decrypted) = aes_cbc_decrypt_vec(&iv_key, &data) {
                        data = decrypted;
                    }
                }
            }

            return Ok(data);
        }

        // Read from PFS filesystem
        let sector_loc = node.loc.ok_or_else(|| anyhow!("Missing sector loc"))? as usize;
        let nblocks = node.blocks.ok_or_else(|| anyhow!("Missing block count"))? as usize;
        let file_size = node.size as usize;

        let mut out = Vec::with_capacity(file_size);
        let mut size_decompressed = 0usize;

        const BUFFER_SIZE: usize = 0x11000;
        let mut pfsc_buffer = vec![0u8; BUFFER_SIZE];
        let mut decrypted_buffer = vec![0u8; BUFFER_SIZE];
        let mut decomp_block = vec![0u8; 0x10000];

        for j in 0..nblocks {
            let sector_idx = sector_loc + j;
            if sector_idx + 1 >= self.sector_map.len() {
                break;
            }

            let sec_offset = self.sector_map[sector_idx];
            let sec_size = (self.sector_map[sector_idx + 1] - sec_offset) as usize;

            let abs_offset = self.pfs_image_offset + (self.pfsc_offset as u64) + sec_offset;
            let sector_mask = (sec_offset + self.pfsc_offset as u64) & !0xFFF;
            let prev_data = ((sec_offset + self.pfsc_offset as u64) - sector_mask) as usize;
            let aligned_file_offset = abs_offset - prev_data as u64;
            let current_sector = sector_mask / 0x1000;

            pkg_file.seek(SeekFrom::Start(aligned_file_offset))?;
            let bytes_read = pkg_file.read(&mut pfsc_buffer)?;
            if bytes_read < BUFFER_SIZE {
                pfsc_buffer[bytes_read..].fill(0);
            }

            decrypt_pfs(&self.data_key, &self.tweak_key, &pfsc_buffer, &mut decrypted_buffer, current_sector);

            let comp_slice = &decrypted_buffer[prev_data..prev_data + sec_size];

            if sec_size == 0x10000 {
                decomp_block.copy_from_slice(comp_slice);
            } else if sec_size < 0x10000 {
                let _ = decompress_pfsc_block(comp_slice, &mut decomp_block);
            }

            size_decompressed += 0x10000;

            if j < nblocks - 1 {
                out.extend_from_slice(&decomp_block);
            } else {
                let excess = size_decompressed.saturating_sub(file_size);
                let write_size = decomp_block.len().saturating_sub(excess);
                out.extend_from_slice(&decomp_block[..write_size]);
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_virtual_fs_on_sample_pkg() {
        let pkg_path = Path::new("/Users/mehdi/projects/fun/rsx4/IV0000-BREW00777_00-RSX4EMULATOR0001.pkg");
        if !pkg_path.exists() {
            return;
        }

        let mut pkg = Pkg::open(pkg_path).expect("Failed to open test PKG");
        let vfs = VirtualFs::build(&mut pkg).expect("Failed to build VirtualFs");

        assert!(!vfs.nodes.is_empty(), "VFS nodes should not be empty");

        // Verify root entries (sce_sys should be in root list)
        let root_entries = vfs.list_dir("");
        assert!(root_entries.iter().any(|n| n.name == "sce_sys"), "sce_sys should be at root");

        // Verify sce_sys entries
        let sce_sys_entries = vfs.list_dir("sce_sys");
        assert!(sce_sys_entries.iter().any(|n| n.name == "param.sfo"), "param.sfo should be in sce_sys");

        // Read param.sfo content on demand
        let param_node = vfs.find_node("sce_sys/param.sfo").expect("param.sfo node not found");
        let param_bytes = vfs.read_file_content(pkg.file_mut(), param_node).expect("Failed to read param.sfo");
        assert!(!param_bytes.is_empty());

        let sfo = super::super::psf::Psf::parse(&param_bytes).expect("Failed to parse PSF");
        assert_eq!(sfo.title_id(), Some("BREW00777"));
    }
}
