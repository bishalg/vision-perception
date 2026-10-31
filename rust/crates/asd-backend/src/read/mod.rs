//! owner: WP-B.3
//!
//! Console read models (API-2): session list/detail, clips, cursor pagination.
//!
//! Handlers authorise with the WP-B.2 [`crate::auth::extractors::Require`] extractor
//! (`perm::SessionRead`, ...) and check the loaded resource's org with `ensure_org`
//! (another org's resource answers 404, AUTH-4).

pub mod clips;
pub mod cursor;
pub mod sessions;
pub mod timeline;
