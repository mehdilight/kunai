//! Key derivation functions (HMAC-SHA256 and SHA-256) for PKG/PFS crypto.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Derives the 32-byte `iv_key` from a 32-byte PKG entry header and the 32-byte `dk3` key.
pub fn derive_iv_key(entry_raw: &[u8; 32], dk3: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(entry_raw);
    hasher.update(dk3);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Derives PFS data_key (16 bytes) and tweak_key (16 bytes) from ekpfs and the 16-byte seed.
///
/// Uses HMAC-SHA256 with `ekpfs` as the key and `[1, 0, 0, 0] || seed` (20 bytes) as the message.
pub fn derive_pfs_keys(
    ekpfs: &[u8; 32],
    seed: &[u8; 16],
) -> ([u8; 16], [u8; 16]) {
    let mut mac = HmacSha256::new_from_slice(ekpfs).expect("HMAC can take 32-byte key");

    // Index = 1 as 32-bit LE (4 bytes)
    let index: [u8; 4] = 1u32.to_le_bytes();
    mac.update(&index);
    mac.update(seed);

    let digest = mac.finalize().into_bytes();

    let mut tweak_key = [0u8; 16];
    let mut data_key = [0u8; 16];

    tweak_key.copy_from_slice(&digest[0..16]);
    data_key.copy_from_slice(&digest[16..32]);

    (data_key, tweak_key)
}
