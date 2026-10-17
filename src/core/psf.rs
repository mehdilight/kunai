//! PSF (`param.sfo`) PlayStation System File parser.

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::collections::HashMap;

pub const PSF_MAGIC: u32 = 0x00505346; // '\0PSF'
pub const PSF_VERSION_1_1: u32 = 0x00000101;
pub const PSF_VERSION_1_0: u32 = 0x00000100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsfEntryFmt {
    Binary = 0x0004,
    Text = 0x0204,
    Integer = 0x0404,
}

#[derive(Debug, Clone)]
pub enum PsfValue {
    Text(String),
    Integer(i32),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Default)]
pub struct Psf {
    pub entries: HashMap<String, PsfValue>,
}

impl Psf {
    pub fn parse(buffer: &[u8]) -> Result<Self> {
        if buffer.len() < 0x14 {
            return Err(anyhow!("PSF buffer too small for header"));
        }

        let magic = BigEndian::read_u32(&buffer[0..4]);
        if magic != PSF_MAGIC {
            return Err(anyhow!("Invalid PSF magic: expected 0x{:08X}, got 0x{:08X}", PSF_MAGIC, magic));
        }

        let version = LittleEndian::read_u32(&buffer[4..8]);
        if version != PSF_VERSION_1_1 && version != PSF_VERSION_1_0 {
            return Err(anyhow!("Unsupported PSF version: 0x{:08X}", version));
        }

        let key_table_offset = LittleEndian::read_u32(&buffer[8..12]) as usize;
        let data_table_offset = LittleEndian::read_u32(&buffer[12..16]) as usize;
        let index_table_entries = LittleEndian::read_u32(&buffer[16..20]) as usize;

        let mut entries = HashMap::new();

        for i in 0..index_table_entries {
            let entry_offset = 0x14 + i * 0x10;
            if entry_offset + 0x10 > buffer.len() {
                break;
            }

            let key_offset = LittleEndian::read_u16(&buffer[entry_offset..entry_offset + 2]) as usize;
            let param_fmt = LittleEndian::read_u16(&buffer[entry_offset + 2..entry_offset + 4]);
            let param_len = LittleEndian::read_u32(&buffer[entry_offset + 4..entry_offset + 8]) as usize;
            let _param_max_len = LittleEndian::read_u32(&buffer[entry_offset + 8..entry_offset + 12]) as usize;
            let data_offset = LittleEndian::read_u32(&buffer[entry_offset + 12..entry_offset + 16]) as usize;

            // Read key from key table
            let k_start = key_table_offset + key_offset;
            if k_start >= buffer.len() {
                continue;
            }
            let k_end = buffer[k_start..]
                .iter()
                .position(|&b| b == 0)
                .map(|p| k_start + p)
                .unwrap_or(buffer.len());
            let key = String::from_utf8_lossy(&buffer[k_start..k_end]).to_string();

            // Read data from data table
            let d_start = data_table_offset + data_offset;
            let d_end = d_start + param_len;
            if d_end > buffer.len() {
                continue;
            }
            let data_slice = &buffer[d_start..d_end];

            let value = match param_fmt {
                0x0204 | 0x0402 => {
                    // Null-terminated text
                    let len = data_slice.iter().position(|&b| b == 0).unwrap_or(data_slice.len());
                    let s = String::from_utf8_lossy(&data_slice[..len]).to_string();
                    PsfValue::Text(s)
                }
                0x0404 => {
                    if data_slice.len() >= 4 {
                        let num = LittleEndian::read_i32(&data_slice[..4]);
                        PsfValue::Integer(num)
                    } else {
                        PsfValue::Integer(0)
                    }
                }
                _ => PsfValue::Binary(data_slice.to_vec()),
            };

            entries.insert(key, value);
        }

        Ok(Self { entries })
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.entries.get(key) {
            Some(PsfValue::Text(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn get_integer(&self, key: &str) -> Option<i32> {
        match self.entries.get(key) {
            Some(PsfValue::Integer(val)) => Some(*val),
            _ => None,
        }
    }

    pub fn get_binary(&self, key: &str) -> Option<&[u8]> {
        match self.entries.get(key) {
            Some(PsfValue::Binary(b)) => Some(b.as_slice()),
            _ => None,
        }
    }

    pub fn title_id(&self) -> Option<&str> {
        self.get_string("TITLE_ID")
    }

    pub fn category(&self) -> Option<&str> {
        self.get_string("CATEGORY")
    }

    pub fn content_id(&self) -> Option<&str> {
        self.get_string("CONTENT_ID")
    }

    pub fn title(&self) -> Option<&str> {
        self.get_string("TITLE")
    }

    pub fn app_ver(&self) -> Option<&str> {
        self.get_string("APP_VER")
    }

    pub fn version(&self) -> Option<&str> {
        self.get_string("VERSION")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psf_empty_fails() {
        assert!(Psf::parse(&[]).is_err());
        assert!(Psf::parse(&[0u8; 10]).is_err());
    }
}

