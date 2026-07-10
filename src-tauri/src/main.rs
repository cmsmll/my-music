//! 桌面应用二进制入口。
//!
//! 具体 Tauri 初始化逻辑在库 crate 的 `run` 中维护。

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 启动 Tauri 应用。
fn main() {
    my_music_lib::run()
}
