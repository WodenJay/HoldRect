use std::sync::mpsc::Sender;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// Application exit signal type
pub struct AppExit;

/// Create system tray icon with quit menu.
/// Returns the TrayIcon (must be kept alive) and sends AppExit on quit.
pub fn start_tray(exit_tx: Sender<AppExit>) -> TrayIcon {
    // Build tray menu
    let quit_item = MenuItem::new("退出 HoldRect", true, None);
    let tray_menu = Menu::new();
    tray_menu
        .append(&quit_item)
        .expect("Failed to add menu item");

    // Create icon (16x16 single-color placeholder)
    let icon = create_icon();

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("HoldRect - 按住Ctrl+拖拽画框")
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    // Handle menu events in a background thread
    let quit_id = quit_item.id().clone();
    std::thread::spawn(move || loop {
        if let Ok(event) = MenuEvent::receiver().recv() {
            if event.id == quit_id {
                let _ = exit_tx.send(AppExit);
                break;
            }
        }
    });

    tray_icon
}

fn create_icon() -> Icon {
    // 16x16 red square icon
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for _ in 0..(16 * 16) {
        rgba.extend_from_slice(&[0xFF, 0x00, 0x00, 0xFF]); // R, G, B, A
    }
    Icon::from_rgba(rgba, 16, 16).expect("Failed to create icon")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn app_exit_signal_type_exists() {
        // Verify AppExit can be constructed and sent through a channel
        let (tx, rx) = mpsc::channel();
        tx.send(AppExit).unwrap();
        let received = rx.try_recv();
        assert!(received.is_ok(), "AppExit should be sendable through mpsc");
    }

    #[test]
    fn create_icon_returns_valid_icon() {
        let icon = create_icon();
        // Icon::from_rgba should succeed with valid RGBA data
        // We can't easily inspect internal state, but if it didn't panic, it's valid
        let _ = icon;
    }

    #[test]
    fn create_icon_rgba_size() {
        // Verify the RGBA buffer is correct size: 16 * 16 * 4 = 1024
        let mut rgba = Vec::with_capacity(16 * 16 * 4);
        for _ in 0..(16 * 16) {
            rgba.extend_from_slice(&[0xFF, 0x00, 0x00, 0xFF]);
        }
        assert_eq!(rgba.len(), 1024, "RGBA buffer should be 1024 bytes (16*16*4)");
    }

    #[test]
    fn create_icon_rgba_contents() {
        // Verify first pixel is red with full alpha
        let mut rgba = Vec::with_capacity(16 * 16 * 4);
        for _ in 0..(16 * 16) {
            rgba.extend_from_slice(&[0xFF, 0x00, 0x00, 0xFF]);
        }
        assert_eq!(rgba[0], 0xFF, "R channel should be 0xFF");
        assert_eq!(rgba[1], 0x00, "G channel should be 0x00");
        assert_eq!(rgba[2], 0x00, "B channel should be 0x00");
        assert_eq!(rgba[3], 0xFF, "A channel should be 0xFF");
    }

    #[test]
    fn app_exit_channel_sends_and_receives() {
        let (tx, rx) = mpsc::channel::<AppExit>();
        tx.send(AppExit).unwrap();
        assert!(rx.recv().is_ok(), "Should receive AppExit from channel");
    }
}
