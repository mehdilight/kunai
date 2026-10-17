//! In-memory virtual filesystem tree for exploring PS4 PKG contents without extracting.

use anyhow::{anyhow, Result};
use byteorder::{ByteOrder, LittleEndian};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use super::crypto::{
    aes_cbc_decrypt_vec, decrypt_dk3, decrypt_ekpfs, decrypt_pfs, derive_iv_key, derive_pfs_keys,
};
use super::entry_types::get_entry_name_by_type;
use super::pfs::{find_pfsc_offset, parse_pfs_tree, process_pfs_block, PfscHeader};
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

        // Parse PFS directory tree hierarchically
        let pfs_image_offset = pkg.header.pfs_image_offset;
        let pfs_entries = parse_pfs_tree(
            pfsc_data,
            pkg.file_mut(),
            pfs_image_offset,
            pfsc_offset,
            &sector_map,
            &data_key,
            &tweak_key,
        )?;

        for entry in pfs_entries {
            if entry.is_dir && entry.relative_path == "sce_sys" {
                // sce_sys is already added as virtual root folder
                continue;
            }

            let parent_path = match entry.relative_path.rfind('/') {
                Some(idx) => entry.relative_path[..idx].to_string(),
                None => "".to_string(),
            };

            nodes.push(FsNode {
                name: entry.name,
                relative_path: entry.relative_path,
                parent_path,
                is_dir: entry.is_dir,
                size: entry.size,
                inode: Some(entry.inode),
                loc: Some(entry.loc),
                blocks: Some(entry.blocks),
                is_sce_sys: false,
                sce_sys_offset: None,
                sce_sys_size: None,
                sce_sys_id: None,
                raw_header: None,
            });
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
        let mut seen = std::collections::HashSet::new();
        let mut list = Vec::new();
        for node in &self.nodes {
            if node.parent_path == parent_path && seen.insert(&node.name) {
                list.push(node);
            }
        }
        // Folders first, then files alphabetically
        list.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
            }
        });
        list
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

        let mut decomp_block = [0u8; 0x10000];

        for j in 0..nblocks {
            process_pfs_block(
                pkg_file,
                self.pfs_image_offset,
                self.pfsc_offset,
                &self.sector_map,
                &self.data_key,
                &self.tweak_key,
                sector_loc + j,
                &mut decomp_block,
            )?;

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

        // Verify root entries (sce_sys, games, sce_module, eboot.bin should be in root list)
        let root_entries = vfs.list_dir("");
        assert!(root_entries.iter().any(|n| n.name == "sce_sys"), "sce_sys should be at root");
        assert!(root_entries.iter().any(|n| n.name == "sce_module"), "sce_module should be at root");
        assert!(root_entries.iter().any(|n| n.name == "games"), "games should be at root");
        assert!(root_entries.iter().any(|n| n.name == "eboot.bin"), "eboot.bin should be at root");

        // Verify sce_module contains .prx files
        let sce_module_entries = vfs.list_dir("sce_module");
        assert!(sce_module_entries.iter().any(|n| n.name == "libc.prx"), "libc.prx missing from sce_module");
        assert!(sce_module_entries.iter().any(|n| n.name == "libSceFios2.prx"), "libSceFios2.prx missing from sce_module");

        // Verify games contains tetris.elf
        let games_entries = vfs.list_dir("games");
        assert!(games_entries.iter().any(|n| n.name == "tetris.elf"), "tetris.elf missing from games");

        // Read param.sfo content on demand
        let param_node = vfs.find_node("sce_sys/param.sfo").expect("param.sfo node not found");
        let param_bytes = vfs.read_file_content(pkg.file_mut(), param_node).expect("Failed to read param.sfo");
        assert!(!param_bytes.is_empty());
        let sfo = super::super::psf::Psf::parse(&param_bytes).expect("Failed to parse PSF");
        assert_eq!(sfo.title_id(), Some("BREW00777"));

        // Read libc.prx content on demand
        let prx_node = vfs.find_node("sce_module/libc.prx").expect("libc.prx node not found");
        let prx_bytes = vfs.read_file_content(pkg.file_mut(), prx_node).expect("Failed to read libc.prx");
        // Verify SELF / PRX magic 0x1D3D154F
        assert_eq!(&prx_bytes[0..4], &[0x4F, 0x15, 0x3D, 0x1D]);
    }
}
