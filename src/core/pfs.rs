//! PFS (PlayStation File System) and PFSC (compressed PFS) parser and unpacker.

use anyhow::{anyhow, Result};
use byteorder::{ByteOrder, LittleEndian};
use flate2::read::ZlibDecoder;
use std::io::Read;

pub const PFS_FILE: u32 = 2;
pub const PFS_DIR: u32 = 3;
pub const PFS_CURRENT_DIR: u32 = 4;
pub const PFS_PARENT_DIR: u32 = 5;

pub const PFSC_MAGIC: u32 = 0x43534650; // "PFSC"

#[derive(Debug, Clone)]
pub struct PfscHeader {
    pub magic: u32,
    pub unk4: u32,
    pub unk8: u32,
    pub block_sz: u32,
    pub block_sz2: u64,
    pub block_offsets: u64,
    pub data_start: u64,
    pub data_length: u64,
}

impl PfscHeader {
    pub fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < 48 {
            return Err(anyhow!("Buffer too small for PFSC header"));
        }

        let magic = LittleEndian::read_u32(&buf[0..4]);
        if magic != PFSC_MAGIC {
            return Err(anyhow!("Invalid PFSC magic: 0x{:08X}", magic));
        }

        Ok(Self {
            magic,
            unk4: LittleEndian::read_u32(&buf[4..8]),
            unk8: LittleEndian::read_u32(&buf[8..12]),
            block_sz: LittleEndian::read_u32(&buf[12..16]),
            block_sz2: LittleEndian::read_u64(&buf[16..24]),
            block_offsets: LittleEndian::read_u64(&buf[24..32]),
            data_start: LittleEndian::read_u64(&buf[32..40]),
            data_length: LittleEndian::read_u64(&buf[40..48]),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Inode {
    pub mode: u16,
    pub nlink: u16,
    pub flags: u32,
    pub size: i64,
    pub size_compressed: i64,
    pub blocks: u32,
    pub loc: u32,
}

impl Inode {
    pub fn parse(buf: &[u8]) -> Self {
        if buf.len() < 104 {
            return Self::default();
        }

        Self {
            mode: LittleEndian::read_u16(&buf[0..2]),
            nlink: LittleEndian::read_u16(&buf[2..4]),
            flags: LittleEndian::read_u32(&buf[4..8]),
            size: LittleEndian::read_i64(&buf[8..16]),
            size_compressed: LittleEndian::read_i64(&buf[16..24]),
            blocks: LittleEndian::read_u32(&buf[96..100]),
            loc: LittleEndian::read_u32(&buf[100..104]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dirent {
    pub ino: i32,
    pub dirent_type: u32,
    pub namelen: usize,
    pub entsize: usize,
    pub name: String,
}

impl Dirent {
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 16 {
            return None;
        }

        let ino = LittleEndian::read_i32(&buf[0..4]);
        let dirent_type = LittleEndian::read_u32(&buf[4..8]);
        let namelen = LittleEndian::read_u32(&buf[8..12]) as usize;
        let entsize = LittleEndian::read_u32(&buf[12..16]) as usize;

        if entsize == 0 {
            return None;
        }

        let name_start = 16;
        let name_end = (name_start + namelen).min(buf.len());
        let name = String::from_utf8_lossy(&buf[name_start..name_end]).to_string();

        Some(Self {
            ino,
            dirent_type,
            namelen,
            entsize,
            name,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PfsFsEntry {
    pub name: String,
    pub inode: u32,
    pub entry_type: u32,
}

/// Scans decrypted PFS image to locate the PFSC header magic 0x43534650.
pub fn find_pfsc_offset(pfs_image: &[u8]) -> Option<usize> {
    let mut offset = 0x20000;
    while offset + 4 <= pfs_image.len() {
        let magic = LittleEndian::read_u32(&pfs_image[offset..offset + 4]);
        if magic == PFSC_MAGIC {
            return Some(offset);
        }
        offset += 0x10000;
    }
    None
}

/// Decompresses zlib-compressed PFSC sector into a 64KB (0x10000) buffer.
pub fn decompress_pfsc_block(compressed: &[u8], decompressed: &mut [u8]) -> Result<usize> {
    let mut decoder = ZlibDecoder::new(compressed);
    let bytes_read = decoder.read(decompressed)?;
    Ok(bytes_read)
}
