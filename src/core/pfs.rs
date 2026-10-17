//! PFS (PlayStation File System) and PFSC (compressed PFS) parser and unpacker.

use anyhow::{anyhow, Result};
use byteorder::{ByteOrder, LittleEndian};
use flate2::read::ZlibDecoder;
use std::fs::File;
use std::io::Read;

use super::crypto::decrypt_pfs;

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

#[cfg(target_os = "macos")]
pub fn enable_fast_io(file: &File) {
    use std::os::unix::io::AsRawFd;
    unsafe {
        libc::fcntl(file.as_raw_fd(), libc::F_RDAHEAD, 1);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn enable_fast_io(_file: &File) {}

pub trait PagedReadFile {
    fn read_at_offset(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize>;
}

impl PagedReadFile for File {
    #[cfg(unix)]
    fn read_at_offset(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
        use std::os::unix::fs::FileExt;
        let mut total = 0;
        while total < buf.len() {
            match self.read_at(&mut buf[total..], offset + total as u64) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(total)
    }

    #[cfg(windows)]
    fn read_at_offset(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
        use std::os::windows::fs::FileExt;
        let mut total = 0;
        while total < buf.len() {
            match self.seek_read(&mut buf[total..], offset + total as u64) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(total)
    }
}

thread_local! {
    static DECOMPRESSOR: std::cell::RefCell<libdeflater::Decompressor> =
        std::cell::RefCell::new(libdeflater::Decompressor::new());

    static SECTOR_BUFFERS: std::cell::RefCell<(Vec<u8>, Vec<u8>)> =
        std::cell::RefCell::new((vec![0u8; 0x11000], vec![0u8; 0x11000]));
}

/// Decompresses zlib-compressed PFSC sector into a 64KB (0x10000) buffer.
/// Uses ARM NEON SIMD accelerated libdeflater with fallback to flate2 ZlibDecoder.
pub fn decompress_pfsc_block(compressed: &[u8], decompressed: &mut [u8]) -> Result<usize> {
    DECOMPRESSOR.with(|d| {
        let mut decompressor = d.borrow_mut();
        match decompressor.zlib_decompress(compressed, decompressed) {
            Ok(bytes) => Ok(bytes),
            Err(_) => {
                let mut decoder = ZlibDecoder::new(compressed);
                let bytes_read = decoder.read(decompressed)?;
                Ok(bytes_read)
            }
        }
    })
}

/// Reads, decrypts, and decompresses a single 64KB PFSC block using zero-allocation scratch buffers.
pub fn process_pfs_block(
    file: &File,
    pfs_image_offset: u64,
    pfsc_offset: usize,
    sector_map: &[u64],
    data_key: &[u8; 16],
    tweak_key: &[u8; 16],
    sector_idx: usize,
    out_decomp_block: &mut [u8; 0x10000],
) -> Result<()> {
    if sector_idx + 1 >= sector_map.len() {
        return Err(anyhow!("Sector index out of bounds in sector_map"));
    }

    let sec_offset = sector_map[sector_idx];
    let sec_size = (sector_map[sector_idx + 1] - sec_offset) as usize;

    let abs_offset = pfs_image_offset + (pfsc_offset as u64) + sec_offset;
    let sector_mask = (sec_offset + pfsc_offset as u64) & !0xFFF;
    let prev_data = ((sec_offset + pfsc_offset as u64) - sector_mask) as usize;
    let aligned_file_offset = abs_offset - prev_data as u64;
    let current_sector = sector_mask / 0x1000;

    const BUFFER_SIZE: usize = 0x11000;
    if prev_data + sec_size > BUFFER_SIZE {
        return Err(anyhow!("Sector size exceeds buffer size"));
    }

    SECTOR_BUFFERS.with(|bufs| -> Result<()> {
        let mut bufs = bufs.borrow_mut();
        let (pfsc_buffer, decrypted_buffer) = &mut *bufs;

        let bytes_read = file.read_at_offset(&mut pfsc_buffer[..BUFFER_SIZE], aligned_file_offset)?;
        if bytes_read < BUFFER_SIZE {
            pfsc_buffer[bytes_read..BUFFER_SIZE].fill(0);
        }

        decrypt_pfs(
            data_key,
            tweak_key,
            &pfsc_buffer[..BUFFER_SIZE],
            &mut decrypted_buffer[..BUFFER_SIZE],
            current_sector,
        );

        let comp_slice = &decrypted_buffer[prev_data..prev_data + sec_size];

        if sec_size == 0x10000 {
            out_decomp_block.copy_from_slice(comp_slice);
        } else if sec_size < 0x10000 {
            let bytes_decompressed = decompress_pfsc_block(comp_slice, out_decomp_block)?;
            if bytes_decompressed < 0x10000 {
                out_decomp_block[bytes_decompressed..].fill(0);
            }
        }

        Ok(())
    })
}
