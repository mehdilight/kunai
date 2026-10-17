//! AES-128-CBC decryption for PKG entries and image keys.

use aes::cipher::{BlockDecryptMut, KeyIvInit, generic_array::GenericArray};
use anyhow::{anyhow, Result};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// Decrypts ciphertext using AES-128-CBC with the key and IV derived from a 32-byte iv_key:
/// - IV:  iv_key[0..16]
/// - Key: iv_key[16..32]
pub fn aes_cbc_decrypt(iv_key: &[u8; 32], ciphertext: &[u8], decrypted: &mut [u8]) -> Result<()> {
    if ciphertext.len() != decrypted.len() {
        return Err(anyhow!("Ciphertext and decrypted buffer lengths do not match"));
    }
    if ciphertext.len() % 16 != 0 {
        return Err(anyhow!("Ciphertext length must be a multiple of 16 bytes"));
    }

    let iv = &iv_key[0..16];
    let key = &iv_key[16..32];

    let mut dec = Aes128CbcDec::new(
        GenericArray::from_slice(key),
        GenericArray::from_slice(iv),
    );

    for (chunk_in, chunk_out) in ciphertext.chunks_exact(16).zip(decrypted.chunks_exact_mut(16)) {
        chunk_out.copy_from_slice(chunk_in);
        dec.decrypt_block_mut(GenericArray::from_mut_slice(chunk_out));
    }

    Ok(())
}

/// Decrypts ciphertext in-place or returns a newly allocated decrypted vector.
pub fn aes_cbc_decrypt_vec(iv_key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let mut padded_len = ciphertext.len();
    if padded_len % 16 != 0 {
        padded_len = (padded_len / 16 + 1) * 16;
    }

    let mut input = ciphertext.to_vec();
    input.resize(padded_len, 0);

    let mut output = vec![0u8; padded_len];
    aes_cbc_decrypt(iv_key, &input, &mut output)?;

    output.truncate(ciphertext.len());
    Ok(output)
}
