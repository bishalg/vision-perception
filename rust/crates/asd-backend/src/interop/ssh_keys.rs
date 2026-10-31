//! owner: WP-I.7
//!
//! SSH keys of SFTP destinations (INTG-5): the server generates one Ed25519 key per
//! destination and keeps only its 32-byte seed (encrypted, `destination::secret_aad`);
//! hospital IT installs the OpenSSH public key. Host keys are pinned by their SHA-256
//! fingerprint in the form `ssh-keygen -lf` prints (`SHA256:<base64 without padding>`).

use base64::Engine as _;
use russh::keys::ssh_key::private::Ed25519Keypair;
use russh::keys::{HashAlg, PrivateKey, PublicKey};
use zeroize::Zeroizing;

/// A freshly generated key: the seed to store and the public half to show.
pub struct GeneratedKey {
    pub seed: Zeroizing<[u8; 32]>,
    /// `ssh-ed25519 AAAA...` (no comment).
    pub public_openssh: String,
    /// `SHA256:<base64>`.
    pub fingerprint: String,
}

impl std::fmt::Debug for GeneratedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeneratedKey")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// The private key of a stored 32-byte seed.
pub fn private_key(seed: &[u8]) -> Option<PrivateKey> {
    let seed: &[u8; 32] = seed.try_into().ok()?;
    Some(PrivateKey::from(Ed25519Keypair::from_seed(seed)))
}

/// `ssh-ed25519 AAAA...` of a public key, without a comment.
pub fn public_openssh(key: &PublicKey) -> Option<String> {
    let line = key.to_openssh().ok()?;
    let mut parts = line.split_whitespace();
    Some(format!("{} {}", parts.next()?, parts.next()?))
}

/// `SHA256:<base64>` of a public key.
pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// Generates a new Ed25519 key from the system CSPRNG.
pub fn generate() -> Result<GeneratedKey, String> {
    let mut seed = Zeroizing::new([0u8; 32]);
    getrandom::fill(seed.as_mut()).map_err(|e| format!("randomness: {e}"))?;
    let key = private_key(seed.as_ref()).ok_or("ed25519 key from seed")?;
    let public_openssh = public_openssh(key.public_key()).ok_or("openssh encoding")?;
    let fingerprint = fingerprint(key.public_key());
    Ok(GeneratedKey {
        seed,
        public_openssh,
        fingerprint,
    })
}

/// The base64 part of a SHA-256 host key fingerprint, accepting `SHA256:<b64>` or the bare
/// base64, with or without padding. `None` unless it decodes to 32 bytes.
pub fn normalise_fingerprint(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let b64 = raw
        .strip_prefix("SHA256:")
        .unwrap_or(raw)
        .trim_end_matches('=');
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(b64)
        .ok()?;
    (bytes.len() == 32).then(|| b64.to_owned())
}

/// Whether `key` is the host key pinned as `pinned` (any form [`normalise_fingerprint`]
/// accepts).
pub fn host_key_matches(key: &PublicKey, pinned: &str) -> bool {
    let Some(want) = normalise_fingerprint(pinned) else {
        return false;
    };
    normalise_fingerprint(&fingerprint(key)).is_some_and(|got| {
        // Constant time is not needed (the fingerprint is public), but be exact.
        got == want
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_round_trip_through_the_seed() {
        let k = generate().unwrap();
        assert!(k.public_openssh.starts_with("ssh-ed25519 "));
        assert!(k.fingerprint.starts_with("SHA256:"));
        let again = private_key(k.seed.as_ref()).unwrap();
        assert_eq!(
            public_openssh(again.public_key()).unwrap(),
            k.public_openssh
        );
        assert!(host_key_matches(again.public_key(), &k.fingerprint));
        assert!(host_key_matches(
            again.public_key(),
            k.fingerprint.trim_start_matches("SHA256:")
        ));
        let other = generate().unwrap();
        assert!(!host_key_matches(again.public_key(), &other.fingerprint));
        assert!(normalise_fingerprint("SHA256:short").is_none());
        assert!(!format!("{k:?}").contains("seed"));
    }
}
