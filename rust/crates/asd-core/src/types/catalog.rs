//! owner: WP-R.0
//!
//! Port of `StimulusCatalog.swift` model types.

use serde::{Deserialize, Serialize};

/// `StimulusCatalog.AgePack`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgePack {
    pub id: String,
    pub min_months: i64,
    pub max_months: i64,
}

/// `StimulusCatalog.Membership`: protocol id and play order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    pub id: String,
    pub order: i64,
}

/// `StimulusCatalog.Area`: an AOI box with clinical roles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Area {
    pub name: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub x_min: f64,
    pub y_min: f64,
    pub x_max: f64,
    pub y_max: f64,
    pub roles: Vec<String>,
}

/// `StimulusCatalog.Clip`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub title: String,
    pub category: String,
    pub duration_sec: f64,
    pub min_months: i64,
    pub max_months: i64,
    pub protocols: Vec<Membership>,
    pub areas: Vec<Area>,
}

/// The stimulus catalog document.
///
/// `comment` is authoring prose present in the bundled file; Swift drops it on decode, Rust
/// keeps it so a catalog round-trips without loss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StimulusCatalog {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub languages: Vec<String>,
    pub fallback_language: String,
    pub age_packs: Vec<AgePack>,
    pub clips: Vec<Clip>,
}
