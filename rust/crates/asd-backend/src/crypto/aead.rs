//! owner: WP-C.9
//!
//! The only AEAD call sites in the service (KEY-6): AES-256-GCM seal/open over the
//! envelope layout of KEY-3,
//!
//! ```text
//! version (1 byte, 0x01) || nonce (12 bytes, random) || ciphertext || tag (16 bytes)
//! ```
//!
//! The nonce is fresh from the system CSPRNG for every encryption. With random 96-bit
//! nonces, NIST SP 800-38D caps one key at 2^32 encryptions; one key per org stays far
//! below that, and rotation (KEY-5) gives a new key id when needed.

use aws_lc_rs::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};

use super::CryptoError;

/// Layout version byte of every ciphertext written by this module.
pub const VERSION: u8 = 1;
/// AES-GCM tag length.
pub const TAG_LEN: usize = 16;
/// Bytes added to the plaintext length.
pub const OVERHEAD: usize = 1 + NONCE_LEN + TAG_LEN;

/// An AES-256-GCM key. aws-lc-rs wipes the key schedule on drop.
pub(crate) struct AeadKey(LessSafeKey);

impl AeadKey {
    pub(crate) fn new(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let unbound = UnboundKey::new(&AES_256_GCM, bytes).map_err(|_| CryptoError::Encrypt)?;
        Ok(Self(LessSafeKey::new(unbound)))
    }

    /// Encrypts `plaintext` bound to `aad`.
    pub(crate) fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce = [0u8; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| CryptoError::Encrypt)?;
        let mut out = Vec::with_capacity(plaintext.len() + OVERHEAD);
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(plaintext);
        let tag = self
            .0
            .seal_in_place_separate_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut out[1 + NONCE_LEN..],
            )
            .map_err(|_| CryptoError::Encrypt)?;
        out.extend_from_slice(tag.as_ref());
        Ok(out)
    }

    /// Decrypts a [`seal`](Self::seal) output; fails on any change to the bytes or the AAD.
    pub(crate) fn open(&self, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if blob.len() < OVERHEAD || blob[0] != VERSION {
            return Err(CryptoError::Malformed);
        }
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&blob[1..1 + NONCE_LEN]);
        let mut in_out = blob[1 + NONCE_LEN..].to_vec();
        let plain = self
            .0
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut in_out,
            )
            .map_err(|_| CryptoError::Decrypt)?;
        Ok(plain.to_vec())
    }
}

/// 32 random bytes from the system CSPRNG (new data keys).
pub(crate) fn random_key() -> Result<[u8; 32], CryptoError> {
    let mut k = [0u8; 32];
    SystemRandom::new()
        .fill(&mut k)
        .map_err(|_| CryptoError::Encrypt)?;
    Ok(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(b: u8) -> AeadKey {
        AeadKey::new(&[b; 32]).unwrap()
    }

    #[test]
    fn layout_and_round_trip() {
        let k = key(1);
        let blob = k.seal(b"aad", b"hello").unwrap();
        assert_eq!(blob[0], VERSION);
        assert_eq!(blob.len(), 5 + OVERHEAD);
        assert_eq!(k.open(b"aad", &blob).unwrap(), b"hello");
        assert_ne!(k.seal(b"aad", b"hello").unwrap(), blob, "random nonce");
        // Empty plaintext is allowed and still authenticated.
        let empty = k.seal(b"aad", b"").unwrap();
        assert_eq!(k.open(b"aad", &empty).unwrap(), b"");
    }

    #[test]
    fn any_change_fails() {
        let k = key(1);
        let blob = k.seal(b"aad", b"hello").unwrap();
        assert!(matches!(k.open(b"aae", &blob), Err(CryptoError::Decrypt)));
        assert!(matches!(
            key(2).open(b"aad", &blob),
            Err(CryptoError::Decrypt)
        ));
        for i in 1..blob.len() {
            let mut bad = blob.clone();
            bad[i] ^= 0x80;
            assert!(
                matches!(k.open(b"aad", &bad), Err(CryptoError::Decrypt)),
                "{i}"
            );
        }
        let mut v = blob.clone();
        v[0] = 0;
        assert!(matches!(k.open(b"aad", &v), Err(CryptoError::Malformed)));
        assert!(matches!(
            k.open(b"aad", &blob[..OVERHEAD - 1]),
            Err(CryptoError::Malformed)
        ));
    }
}
