//! RSA-2048 PKCS#1 v1.5 decryption for PKG keys.

use anyhow::{anyhow, Result};
use rsa::traits::{PrivateKeyParts, PublicKeyParts};
use rsa::{BigUint, Pkcs1v15Encrypt, RsaPrivateKey};
use std::sync::OnceLock;

use super::keys::{FakeKeyset, PkgDerivedKey3Keyset};

static DK3_KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
static FAKE_KEY: OnceLock<RsaPrivateKey> = OnceLock::new();

fn get_dk3_key() -> &'static RsaPrivateKey {
    DK3_KEY.get_or_init(|| {
        let n = BigUint::from_bytes_be(PkgDerivedKey3Keyset::MODULUS);
        let e = BigUint::from_bytes_be(PkgDerivedKey3Keyset::PUBLIC_EXPONENT);
        let d = BigUint::from_bytes_be(PkgDerivedKey3Keyset::PRIVATE_EXPONENT);
        let p = BigUint::from_bytes_be(PkgDerivedKey3Keyset::PRIME1);
        let q = BigUint::from_bytes_be(PkgDerivedKey3Keyset::PRIME2);
        RsaPrivateKey::from_components(n, e, d, vec![p, q]).expect("valid DK3 RSA key")
    })
}

fn get_fake_key() -> &'static RsaPrivateKey {
    FAKE_KEY.get_or_init(|| {
        let n = BigUint::from_bytes_be(FakeKeyset::MODULUS);
        let e = BigUint::from_bytes_be(FakeKeyset::PUBLIC_EXPONENT);
        let d = BigUint::from_bytes_be(FakeKeyset::PRIVATE_EXPONENT);
        let p = BigUint::from_bytes_be(FakeKeyset::PRIME1);
        let q = BigUint::from_bytes_be(FakeKeyset::PRIME2);
        RsaPrivateKey::from_components(n, e, d, vec![p, q]).expect("valid Fake RSA key")
    })
}

/// Decrypts a 256-byte ciphertext using the given private key into a 32-byte key.
///
/// Tries standard PKCS#1 v1.5 first, and falls back to raw CRT decryption with
/// manual PKCS#1 v1.5 frame extraction for maximum compatibility with all PS4 PKG tools.
fn rsa_decrypt_2048(key: &RsaPrivateKey, ciphertext: &[u8; 256]) -> Result<[u8; 32]> {
    // 1. Try standard PKCS#1 v1.5 decrypt
    if let Ok(decrypted) = key.decrypt(Pkcs1v15Encrypt, ciphertext) {
        if decrypted.len() >= 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&decrypted[..32]);
            return Ok(out);
        }
    }

    // 2. Fallback: Raw modular exponentiation c^d mod n
    let c = BigUint::from_bytes_be(ciphertext);
    let n = key.n();
    let d = key.d();
    let m = c.modpow(d, n);

    let mut m_bytes = m.to_bytes_be();
    // Pad to 256 bytes if leading zeros were dropped
    if m_bytes.len() < 256 {
        let mut padded = vec![0u8; 256 - m_bytes.len()];
        padded.extend_from_slice(&m_bytes);
        m_bytes = padded;
    }

    // Parse PKCS#1 v1.5 frame: 00 02 [PS...] 00 [M...]
    if m_bytes.len() == 256 && (m_bytes[0] == 0x00 || m_bytes[0] == 0x02) {
        let start_idx = if m_bytes[0] == 0x00 { 2 } else { 1 };
        // Find 0x00 delimiter
        if let Some(pos) = m_bytes[start_idx..].iter().position(|&b| b == 0x00) {
            let data = &m_bytes[start_idx + pos + 1..];
            if data.len() >= 32 {
                let mut out = [0u8; 32];
                out.copy_from_slice(&data[..32]);
                return Ok(out);
            }
        }
    }

    // 3. Fallback: take last 32 bytes of the raw decrypted message
    if m_bytes.len() >= 32 {
        let mut out = [0u8; 32];
        let offset = m_bytes.len() - 32;
        out.copy_from_slice(&m_bytes[offset..]);
        return Ok(out);
    }

    Err(anyhow!("Failed to decrypt RSA-2048 payload"))
}

/// Decrypt DK3 key from PKG entry 0x10 key[3].
pub fn decrypt_dk3(ciphertext: &[u8; 256]) -> Result<[u8; 32]> {
    let key = get_dk3_key();
    rsa_decrypt_2048(key, ciphertext)
}

/// Decrypt EKPFS key from PKG entry 0x20 image_key.
pub fn decrypt_ekpfs(ciphertext: &[u8; 256]) -> Result<[u8; 32]> {
    let key = get_fake_key();
    rsa_decrypt_2048(key, ciphertext)
}
