//! fluxxx-core — platform-independent logic shared by the Tauri app.
//!
//! Everything here is free of Tauri, OS, GUI, and network I/O so it can be
//! unit-tested on any platform (the Windows-only pieces live in the `fluxxx`
//! app crate). Modules:
//!   * [`model`]   — normalized domain types persisted in SQLite / shown in UI.
//!   * [`xtream`]  — Xtream Codes `player_api.php` URL building + response parsing.
//!   * [`country`] — infer a country from an IPTV category name (for roll-ups).
//!   * [`curation`]— decide what is "enabled" and therefore fetched/cached.

pub mod client;
pub mod config;
pub mod country;
pub mod curation;
pub mod db;
pub mod http;
pub mod model;
pub mod xtream;

pub use country::{infer_country, Country};
pub use model::{Category, Channel, EpgProgram, Provider};

// Re-exported so the app crate can name the connection type without depending on
// rusqlite directly (feature unification keeps the `bundled` SQLite).
pub use rusqlite;
