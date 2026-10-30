//! owner: WP-R.3
//!
//! Config bundle signing and verification (CON-6) and RFC 8785 (JCS) canonicalisation.
//!
//! Signature = Ed25519 over `JCS(bundle without "signature")`, written as `ed25519:` +
//! base64url without padding. Verification works on the received JSON document, not on a
//! re-serialised typed value, so a member this build does not know is still covered by
//! the signature. [`verify_bundle`] / [`verify_bundle_with_keys`] are the only verifiers
//! apps (through `asd-ffi`) and the backend use.

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use ed25519_dalek::{Signature, Signer};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::ConfigBundle;

pub use ed25519_dalek::{SigningKey, VerifyingKey};

/// Prefix of every bundle signature.
pub const SIGNATURE_PREFIX: &str = "ed25519:";
/// Name of the member excluded from the signed payload.
pub const SIGNATURE_FIELD: &str = "signature";

/// Why signing or verification failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SigningError {
    #[error("bundle is not valid JSON: {0}")]
    InvalidJson(String),
    #[error("bundle is not a JSON object")]
    NotAnObject,
    #[error("bundle has no `signature` string")]
    MissingSignature,
    #[error("signature is not `ed25519:` + base64url of 64 bytes")]
    MalformedSignature,
    #[error("signature does not match the bundle")]
    BadSignature,
    #[error("no trusted key with key_id `{0}`")]
    UnknownKeyId(String),
    #[error("bundle has no `key_id` string")]
    MissingKeyId,
    #[error("signed bundle does not match the ConfigBundle contract: {0}")]
    InvalidBundle(String),
    #[error("invalid key: {0}")]
    InvalidKey(String),
    #[error("canonicalisation failed: {0}")]
    Canonicalisation(String),
}

/// RFC 8785 canonical bytes of any serialisable value (sorted keys by UTF-16 code units,
/// ECMAScript number formatting, minimal string escaping, no whitespace). Use this, not
/// `serde_jcs` 0.1, whose member order is wrong for keys that need escaping or lie outside
/// the BMP (see `jcs.rs`).
pub fn canonical_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, SigningError> {
    let value =
        serde_json::to_value(value).map_err(|e| SigningError::Canonicalisation(e.to_string()))?;
    Ok(crate::jcs::to_vec(&value))
}

/// The bytes that are signed: JCS of `bundle` with the `signature` member removed.
pub fn signing_payload(bundle: &Value) -> Result<Vec<u8>, SigningError> {
    let Value::Object(map) = bundle else {
        return Err(SigningError::NotAnObject);
    };
    let mut map = map.clone();
    map.remove(SIGNATURE_FIELD);
    canonical_bytes(&Value::Object(map))
}

/// `ed25519:` + base64url (no padding) of a signature.
pub fn encode_signature(signature: &Signature) -> String {
    format!(
        "{SIGNATURE_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    )
}

fn decode_signature(raw: &str) -> Result<Signature, SigningError> {
    let body = raw
        .strip_prefix(SIGNATURE_PREFIX)
        .ok_or(SigningError::MalformedSignature)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| SigningError::MalformedSignature)?;
    let bytes: [u8; 64] = bytes
        .try_into()
        .map_err(|_| SigningError::MalformedSignature)?;
    Ok(Signature::from_bytes(&bytes))
}

/// Signs `bundle` in place: sets `signature` over the current content (including
/// `key_id`, which the caller sets first).
pub fn sign_bundle(bundle: &mut ConfigBundle, key: &SigningKey) -> Result<(), SigningError> {
    let value =
        serde_json::to_value(&*bundle).map_err(|e| SigningError::InvalidBundle(e.to_string()))?;
    let payload = signing_payload(&value)?;
    bundle.signature = encode_signature(&key.sign(&payload));
    Ok(())
}

/// Signs a bundle given as JSON (any `signature` member is replaced) and returns the
/// signed document as compact JSON. The document must decode as a [`ConfigBundle`].
pub fn sign_bundle_json(bundle_json: &str, key: &SigningKey) -> Result<String, SigningError> {
    let mut value: Value =
        serde_json::from_str(bundle_json).map_err(|e| SigningError::InvalidJson(e.to_string()))?;
    if !value.is_object() {
        return Err(SigningError::NotAnObject);
    }
    value[SIGNATURE_FIELD] = Value::String(format!("{SIGNATURE_PREFIX}A"));
    serde_json::from_value::<ConfigBundle>(value.clone())
        .map_err(|e| SigningError::InvalidBundle(e.to_string()))?;
    let payload = signing_payload(&value)?;
    value[SIGNATURE_FIELD] = Value::String(encode_signature(&key.sign(&payload)));
    serde_json::to_string(&value).map_err(|e| SigningError::InvalidJson(e.to_string()))
}

fn parse_document(bundle_json: &str) -> Result<(Value, Signature), SigningError> {
    let value: Value =
        serde_json::from_str(bundle_json).map_err(|e| SigningError::InvalidJson(e.to_string()))?;
    let Value::Object(map) = &value else {
        return Err(SigningError::NotAnObject);
    };
    let raw = map
        .get(SIGNATURE_FIELD)
        .and_then(Value::as_str)
        .ok_or(SigningError::MissingSignature)?;
    let signature = decode_signature(raw)?;
    Ok((value, signature))
}

fn check(
    value: Value,
    signature: &Signature,
    key: &VerifyingKey,
) -> Result<ConfigBundle, SigningError> {
    let payload = signing_payload(&value)?;
    key.verify_strict(&payload, signature)
        .map_err(|_| SigningError::BadSignature)?;
    serde_json::from_value(value).map_err(|e| SigningError::InvalidBundle(e.to_string()))
}

/// Verifies a bundle against one trusted public key and returns the decoded bundle.
pub fn verify_bundle(bundle_json: &str, key: &VerifyingKey) -> Result<ConfigBundle, SigningError> {
    let (value, signature) = parse_document(bundle_json)?;
    check(value, &signature, key)
}

/// Verifies a bundle against a key ring of `(key_id, key)` pairs, choosing the key named
/// by the bundle's `key_id` (rotation: old and new keys are trusted side by side).
pub fn verify_bundle_with_keys(
    bundle_json: &str,
    keys: &[(&str, VerifyingKey)],
) -> Result<ConfigBundle, SigningError> {
    let (value, signature) = parse_document(bundle_json)?;
    let key_id = value
        .get("key_id")
        .and_then(Value::as_str)
        .ok_or(SigningError::MissingKeyId)?;
    let key = keys
        .iter()
        .find(|(id, _)| *id == key_id)
        .map(|(_, k)| *k)
        .ok_or_else(|| SigningError::UnknownKeyId(key_id.to_string()))?;
    check(value, &signature, &key)
}

/// Verifies with a base64 or base64url public key (the FFI form).
pub fn verify_bundle_b64(
    bundle_json: &str,
    public_key_b64: &str,
) -> Result<ConfigBundle, SigningError> {
    verify_bundle(bundle_json, &parse_public_key(public_key_b64)?)
}

fn decode_b64_any(raw: &str) -> Option<Vec<u8>> {
    let raw = raw.trim();
    [&URL_SAFE_NO_PAD, &URL_SAFE, &STANDARD, &STANDARD_NO_PAD]
        .iter()
        .find_map(|engine| engine.decode(raw).ok())
}

/// Parses a 32-byte Ed25519 public key from base64url or standard base64, padded or not.
pub fn parse_public_key(raw: &str) -> Result<VerifyingKey, SigningError> {
    let bytes = decode_b64_any(raw).ok_or_else(|| SigningError::InvalidKey("not base64".into()))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| SigningError::InvalidKey("public key must be 32 bytes".into()))?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| SigningError::InvalidKey(e.to_string()))
}

/// Public key as base64url without padding (43 characters).
pub fn encode_public_key(key: &VerifyingKey) -> String {
    URL_SAFE_NO_PAD.encode(key.as_bytes())
}

/// Parses a signing key from PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`) or from base64 /
/// base64url of the 32-byte seed (`ASD_BUNDLE_SIGNING_KEY`, CFG-1).
pub fn parse_signing_key(raw: &str) -> Result<SigningKey, SigningError> {
    let raw = raw.trim();
    if raw.starts_with("-----BEGIN") {
        return SigningKey::from_pkcs8_pem(raw)
            .map_err(|e| SigningError::InvalidKey(e.to_string()));
    }
    let bytes = decode_b64_any(raw).ok_or_else(|| SigningError::InvalidKey("not base64".into()))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| SigningError::InvalidKey("seed must be 32 bytes".into()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Signing key from a 32-byte seed (callers supply the randomness).
pub fn signing_key_from_seed(seed: [u8; 32]) -> SigningKey {
    SigningKey::from_bytes(&seed)
}

/// PKCS#8 PEM encoding of a signing key.
pub fn signing_key_to_pkcs8_pem(key: &SigningKey) -> Result<String, SigningError> {
    key.to_pkcs8_pem(Default::default())
        .map(|pem| pem.to_string())
        .map_err(|e| SigningError::InvalidKey(e.to_string()))
}

/// Default key id: `ed25519-` + the first 8 bytes of SHA-256(public key) in hex.
pub fn key_id(key: &VerifyingKey) -> String {
    let digest = Sha256::digest(key.as_bytes());
    let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("ed25519-{hex}")
}
