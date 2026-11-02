//! owner: WP-B.19
//!
//! Follow-ups (REC-4): `GET /orgs/{id}/follow-ups`.
//!
//! Wave 2 step 0 stub: registered in [`super::v1`] and empty until WP-B.19 fills it, so the
//! wave 2a packages never edit `routes/mod.rs`. Its paths answer `404` until then.

use axum::Router;

use crate::app::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}
