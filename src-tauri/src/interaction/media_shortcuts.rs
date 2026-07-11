//! 系统媒体热键交互。
//!
//! 这个模块负责把系统级播放/上一首/下一首按键注册到 Tauri，并把按键事件
//! 转发给前端。实际播放状态仍由前端播放器维护，后端这里只做事件桥接。

use crate::logger::{self, LogKind};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const MEDIA_PLAY_PAUSE_EVENT: &str = "media-play-pause";
const MEDIA_PREVIOUS_EVENT: &str = "media-previous";
const MEDIA_NEXT_EVENT: &str = "media-next";
const DESKTOP_LYRICS_UNLOCK_EVENT: &str = "desktop-lyrics-unlock";
static MEDIA_SHORTCUTS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// 判断是否为主窗口最小化/显示快捷键。
fn is_toggle_window_shortcut(shortcut: &Shortcut) -> bool {
    shortcut.key == Code::Backslash && shortcut.mods == Modifiers::CONTROL
}

/// 判断是否为桌面歌词解锁快捷键。
fn is_unlock_desktop_lyrics_shortcut(shortcut: &Shortcut) -> bool {
    shortcut.key == Code::Backslash && shortcut.mods == Modifiers::ALT
}

/// 将系统快捷键映射为前端监听的事件名。
fn media_shortcut_event(shortcut: &Shortcut) -> Option<&'static str> {
    match shortcut.key {
        Code::MediaPlayPause => Some(MEDIA_PLAY_PAUSE_EVENT),
        Code::MediaTrackPrevious => Some(MEDIA_PREVIOUS_EVENT),
        Code::MediaTrackNext => Some(MEDIA_NEXT_EVENT),
        _ => None,
    }
}

/// 创建全局媒体热键插件。
///
/// 注意：这里仅处理 `Pressed` 状态，避免一次按键触发按下和释放两次事件。
pub(crate) fn media_shortcut_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }

            if is_toggle_window_shortcut(shortcut) {
                toggle_main_window(app);
                return;
            }

            if is_unlock_desktop_lyrics_shortcut(shortcut) {
                unlock_desktop_lyrics(app);
                return;
            }

            if let Some(event_name) = media_shortcut_event(shortcut) {
                let _ = app.emit(event_name, shortcut.to_string());
            }
        })
        .build()
}

/// 注册系统媒体热键。
///
/// 注意：该函数可能被前端重复调用，所以使用原子标记保证同一进程只注册一次。
pub(crate) fn register_media_shortcuts<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if MEDIA_SHORTCUTS_REGISTERED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let shortcuts = [
        Shortcut::new(None, Code::MediaPlayPause),
        Shortcut::new(None, Code::MediaTrackPrevious),
        Shortcut::new(None, Code::MediaTrackNext),
        Shortcut::new(Some(Modifiers::CONTROL), Code::Backslash),
        Shortcut::new(Some(Modifiers::ALT), Code::Backslash),
    ];

    for shortcut in shortcuts {
        if let Err(error) = app.global_shortcut().register(shortcut) {
            logger::error(
                LogKind::App,
                format!("无法注册系统媒体热键 | 快捷键={shortcut} | 原因=\"{error}\""),
            );
        }
    }
}

/// 切换主窗口最小化/显示状态。
fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        logger::warn(
            LogKind::App,
            "全局快捷键切换窗口失败 | 原因=\"未找到 main 窗口\"",
        );
        return;
    };

    match (window.is_visible(), window.is_minimized()) {
        (Ok(true), Ok(false)) => {
            if let Err(error) = window.minimize() {
                logger::warn(
                    LogKind::App,
                    format!("全局快捷键最小化窗口失败 | 原因=\"{error}\""),
                );
            }
        }
        (Ok(_), Ok(_)) => {
            if let Err(error) = window.unminimize() {
                logger::warn(
                    LogKind::App,
                    format!("全局快捷键取消最小化失败 | 原因=\"{error}\""),
                );
            }
            if let Err(error) = window.show() {
                logger::warn(
                    LogKind::App,
                    format!("全局快捷键显示窗口失败 | 原因=\"{error}\""),
                );
            }
            if let Err(error) = window.set_focus() {
                logger::warn(
                    LogKind::App,
                    format!("全局快捷键聚焦窗口失败 | 原因=\"{error}\""),
                );
            }
        }
        (Err(error), _) | (_, Err(error)) => logger::warn(
            LogKind::App,
            format!("全局快捷键读取窗口状态失败 | 原因=\"{error}\""),
        ),
    }
}

/// 只通知桌面歌词窗口解除点击穿透。
fn unlock_desktop_lyrics<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("desktop_lyrics") else {
        logger::warn(
            LogKind::App,
            "桌面歌词解锁失败 | 原因=\"未找到 desktop_lyrics 窗口\"",
        );
        return;
    };

    if let Err(error) = window.emit(DESKTOP_LYRICS_UNLOCK_EVENT, ()) {
        logger::warn(
            LogKind::App,
            format!("桌面歌词解锁事件发送失败 | 原因=\"{error}\""),
        );
    }
}
