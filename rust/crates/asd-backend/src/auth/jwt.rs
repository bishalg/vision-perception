//! owner: WP-B.2
//!
//! AUTH-2 access tokens: JWT, `alg = EdDSA` (Ed25519), 15 minutes, claims
//! `sub, org, role, jti, exp, iat`. The key is Ed25519 PKCS#8 (`asd-cli bundle keygen`
//! output) or a base64 32-byte seed, and must not be the bundle signing key. Only
//! `EdDSA` is accepted on verification (no `none`, no HMAC confusion).

use std::fmt;

use asd_contract::api_v1::Role;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

/// Access-token lifetime (`TokenPair.expires_in`).
pub const ACCESS_TOKEN_TTL_SECS: u32 = 900;

/// The claim set, exactly AUTH-2. `org` is `null` for the global `superadmin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub org: Option<String>,
    pub role: Role,
    pub jti: String,
    pub exp: i64,
    pub iat: i64,
}

/// Signing and verification keys derived from one Ed25519 private key.
#[derive(Clone)]
pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    public: [u8; 32],
    key_id: String,
}

impl JwtKeys {
    /// Parses `ASD_JWT_SIGNING_KEY` (PKCS#8 PEM or base64 seed). The error text never
    /// contains key material.
    pub fn from_key_str(raw: &str) -> Result<Self, String> {
        let signing = asd_contract::signing::parse_signing_key(raw)
            .map_err(|_| "expected an Ed25519 PKCS#8 PEM or a base64 32-byte seed".to_owned())?;
        let der = signing
            .to_pkcs8_der()
            .map_err(|_| "cannot encode the key as PKCS#8".to_owned())?;
        let verifying = signing.verifying_key();
        let public = verifying.to_bytes();
        Ok(Self {
            encoding: EncodingKey::from_ed_der(der.as_bytes()),
            decoding: DecodingKey::from_ed_der(&public),
            public,
            key_id: asd_contract::signing::key_id(&verifying),
        })
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public
    }

    /// `kid` header value (`ed25519-<8 bytes of SHA-256(public key)>`), for rotation.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// A 15-minute token issued now.
    pub fn issue(
        &self,
        user_id: Uuid,
        org_id: Option<Uuid>,
        role: Role,
    ) -> Result<(String, Claims), AppError> {
        self.issue_at(user_id, org_id, role, chrono::Utc::now().timestamp())
    }

    /// A 15-minute token issued at `iat` (Unix seconds); tests use it for expiry.
    pub fn issue_at(
        &self,
        user_id: Uuid,
        org_id: Option<Uuid>,
        role: Role,
        iat: i64,
    ) -> Result<(String, Claims), AppError> {
        let claims = Claims {
            sub: user_id.to_string(),
            org: org_id.map(|o| o.to_string()),
            role,
            jti: Uuid::new_v4().to_string(),
            exp: iat + i64::from(ACCESS_TOKEN_TTL_SECS),
            iat,
        };
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.key_id.clone());
        let token = jsonwebtoken::encode(&header, &claims, &self.encoding)
            .map_err(|e| AppError::Internal(format!("jwt encode: {e}")))?;
        Ok((token, claims))
    }

    /// The claims of a valid, unexpired token signed by this key; `None` otherwise.
    pub fn verify(&self, token: &str) -> Option<Claims> {
        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.algorithms = vec![Algorithm::EdDSA];
        validation.leeway = 5;
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp", "iat", "sub"]);
        jsonwebtoken::decode::<Claims>(token, &self.decoding, &validation)
            .ok()
            .map(|data| data.claims)
    }
}

impl PartialEq for JwtKeys {
    fn eq(&self, other: &Self) -> bool {
        self.public == other.public
    }
}

impl fmt::Debug for JwtKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JwtKeys")
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    fn keys(seed: u8) -> JwtKeys {
        let key = asd_contract::signing::signing_key_from_seed([seed; 32]);
        let pem = asd_contract::signing::signing_key_to_pkcs8_pem(&key).unwrap();
        JwtKeys::from_key_str(&pem).unwrap()
    }

    #[test]
    fn issues_eddsa_tokens_with_the_auth2_claims() {
        let k = keys(1);
        let (user, org) = (Uuid::new_v4(), Uuid::new_v4());
        let (token, claims) = k.issue(user, Some(org), Role::Reviewer).unwrap();
        let header: serde_json::Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(token.split('.').next().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(header["alg"], "EdDSA");
        assert_eq!(header["kid"], k.key_id());
        assert_eq!(claims.exp - claims.iat, 900);
        assert_eq!(k.verify(&token), Some(claims.clone()));
        let payload: serde_json::Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(token.split('.').nth(1).unwrap())
                .unwrap(),
        )
        .unwrap();
        let mut names: Vec<&str> = payload
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        names.sort_unstable();
        assert_eq!(names, ["exp", "iat", "jti", "org", "role", "sub"]);
        assert_eq!(payload["role"], "reviewer");
    }

    #[test]
    fn rejects_expired_foreign_and_tampered_tokens() {
        let k = keys(1);
        let user = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();
        let (old, _) = k
            .issue_at(user, None, Role::Superadmin, now - 901 - 10)
            .unwrap();
        assert!(k.verify(&old).is_none(), "expired");
        let (foreign, _) = keys(2).issue(user, None, Role::Superadmin).unwrap();
        assert!(k.verify(&foreign).is_none(), "other key");
        let (good, _) = k.issue(user, None, Role::Viewer).unwrap();
        let mut parts: Vec<&str> = good.split('.').collect();
        let forged = URL_SAFE_NO_PAD.encode(
            format!(
                r#"{{"sub":"{user}","org":null,"role":"superadmin","jti":"x","exp":{},"iat":{now}}}"#,
                now + 900
            )
            .as_bytes(),
        );
        parts[1] = &forged;
        assert!(k.verify(&parts.join(".")).is_none(), "tampered");
        let none = format!(
            "{}.{}.",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#),
            forged
        );
        assert!(k.verify(&none).is_none(), "alg none");
        assert!(k.verify("garbage").is_none());
    }

    #[test]
    fn seed_and_pem_forms_of_one_key_are_equal_and_debug_hides_the_key() {
        let key = asd_contract::signing::signing_key_from_seed([9u8; 32]);
        let pem = asd_contract::signing::signing_key_to_pkcs8_pem(&key).unwrap();
        let seed = base64::engine::general_purpose::STANDARD.encode([9u8; 32]);
        let a = JwtKeys::from_key_str(&pem).unwrap();
        let b = JwtKeys::from_key_str(&seed).unwrap();
        assert_eq!(a, b);
        let (t, _) = a.issue(Uuid::new_v4(), None, Role::Superadmin).unwrap();
        assert!(b.verify(&t).is_some());
        let dbg = format!("{a:?}");
        assert!(
            dbg.contains("ed25519-") && !dbg.contains("PRIVATE"),
            "{dbg}"
        );
        assert!(JwtKeys::from_key_str("nope").is_err());
    }
}
