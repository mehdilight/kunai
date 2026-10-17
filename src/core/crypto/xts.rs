//! AES-128-XTS sector decryption for PFS image and file blocks.
//!
//! Follows the exact PlayStation 4 PFS XTS specification and polynomial feedback.

use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit, generic_array::GenericArray};
use aes::Aes128;

#[inline]
fn xts_mult(tweak: &mut [u8; 16]) {
    let mut feedback = 0u8;
    for byte in tweak.iter_mut() {
        let tmp = (*byte >> 7) & 1;
        *byte = (*byte << 1) | feedback;
        feedback = tmp;
    }
    if feedback != 0 {
        tweak[0] ^= 0x87;
    }
}

/// Decrypts PFS blocks using AES-128-XTS mode.
///
/// Each sector is 0x1000 (4096) bytes.
/// - `data_key`: 16-byte key for AES block decryption
/// - `tweak_key`: 16-byte key for tweak encryption
/// - `src`: input ciphertext (must be multiple of 0x1000)
/// - `dst`: output plaintext (same size as src)
/// - `sector`: starting 64-bit sector index
pub fn decrypt_pfs(
    data_key: &[u8; 16],
    tweak_key: &[u8; 16],
    src: &[u8],
    dst: &mut [u8],
    sector: u64,
) {
    assert_eq!(src.len(), dst.len());
    let tweak_cipher = Aes128::new(GenericArray::from_slice(tweak_key));
    let data_cipher = Aes128::new(GenericArray::from_slice(data_key));

    for (sector_idx, (src_sector, dst_sector)) in src
        .chunks_exact(0x1000)
        .zip(dst.chunks_exact_mut(0x1000))
        .enumerate()
    {
        let current_sector = sector + sector_idx as u64;

        // Initialize tweak: 64-bit little endian sector index, padded with zeros to 16 bytes
        let mut tweak = [0u8; 16];
        tweak[0..8].copy_from_slice(&current_sector.to_le_bytes());

        // Encrypt the initial tweak
        let mut encrypted_tweak = tweak;
        tweak_cipher.encrypt_block(GenericArray::from_mut_slice(&mut encrypted_tweak));

        // Process 16-byte AES blocks within the 4096-byte sector
        for (src_block, dst_block) in src_sector
            .chunks_exact(16)
            .zip(dst_sector.chunks_exact_mut(16))
        {
            let mut xor_buf = [0u8; 16];
            for i in 0..16 {
                xor_buf[i] = src_block[i] ^ encrypted_tweak[i];
            }

            data_cipher.decrypt_block(GenericArray::from_mut_slice(&mut xor_buf));

            for i in 0..16 {
                dst_block[i] = xor_buf[i] ^ encrypted_tweak[i];
            }

            xts_mult(&mut encrypted_tweak);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xts_mult_polynomial() {
        let mut tweak = [0u8; 16];
        tweak[15] = 0x80;
        xts_mult(&mut tweak);
        // Bit 127 of the 128-bit block overflows, reducing modulo x^128 + x^7 + x^2 + x + 1
        assert_eq!(tweak[0], 0x87);
        assert_eq!(tweak[15], 0x00);
    }
}

