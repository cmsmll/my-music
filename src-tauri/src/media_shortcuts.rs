use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

const MEDIA_PLAY_PAUSE_EVENT: &str = "media-play-pause";
const MEDIA_PREVIOUS_EVENT: &str = "media-previous";
const MEDIA_NEXT_EVENT: &str = "media-next";
static MEDIA_SHORTCUTS_REGISTERED: AtomicBool = AtomicBool::new(false);
fn media_shortcut_event(shortcut: &Shortcut) -> Option<&'static str> {
    match shortcut.key {
        Code::MediaPlayPause => Some(MEDIA_PLAY_PAUSE_EVENT),
        Code::MediaTrackPrevious => Some(MEDIA_PREVIOUS_EVENT),
        Code::MediaTrackNext => Some(MEDIA_NEXT_EVENT),
        _ => None,
    }
}

pub(crate) fn media_shortcut_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }

            if let Some(event_name) = media_shortcut_event(shortcut) {
                let _ = app.emit(event_name, shortcut.to_string());
            }
        })
        .build()
}

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
    ];

    for shortcut in shortcuts {
        if let Err(error) = app.global_shortcut().register(shortcut) {
            eprintln!("无法注册系统媒体热键 {shortcut}: {error}");
        }
    }
}
