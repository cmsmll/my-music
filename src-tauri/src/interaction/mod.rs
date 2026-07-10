//! 前端交互层。
//!
//! 这里集中放置 Tauri command 入口，以及 command 需要直接返回给前端的数据结构。

pub(crate) mod commands;
pub(crate) mod config;
mod decoder;
mod library;
mod lyrics;
pub(crate) mod media_shortcuts;
pub(crate) mod models;
mod playlist;
mod statistics;

pub(crate) use lyrics::LyricsSearchService;
