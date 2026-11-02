//! owner: WP-I.13
//!
//! Share links (SHR-8, SHR-9, HIP-8, HIP-9): `DELETE /share-links/{id}` and the public
//! `/share/{id}/open`, `/share/{id}/file` sub-router (no bearer; `no-store`, `X-Robots-Tag`).
//!
//! Wave 2 step 0 stub: registered in [`super::v1`] and empty until WP-I.13 fills it, so the
//! wave 2a packages never edit `routes/mod.rs`. Its paths answer `404` until then.

use axum::Router;

use crate::app::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}
