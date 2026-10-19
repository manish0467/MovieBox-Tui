//! MovieBox core engine.
//!
//! The crate is split in two layers:
//!
//! * **Core** (`cache`, `config`, `download`, `favorites`, `history`,
//!   `logging`, `models`, `net`, `providers`, `service`, `util`) — platform
//!   agnostic provider clients, aggregation, persistence and downloads. It has
//!   no terminal dependency and is what the Android app links against.
//! * **Terminal frontend** (`tui`, `player`, `proxy`, `updater`) — the
//!   ratatui/crossterm application. It is compiled only when the `tui`
//!   feature is enabled (default).

pub mod cache;
pub mod config;
pub mod download;
pub mod favorites;
pub mod history;
pub mod logging;
pub mod models;
pub mod net;
pub mod providers;
pub mod service;
pub mod util;

#[cfg(feature = "tui")]
pub mod player;
#[cfg(feature = "tui")]
pub mod proxy;
#[cfg(feature = "tui")]
pub mod tui;
#[cfg(feature = "tui")]
pub mod updater;
