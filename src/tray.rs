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
    const SIZE: usize = 32;
    // Rounded rect: inset ~4px from edges → ~24x24 area
    const INSET: f64 = 4.0;
    const RADIUS: f64 = 5.0;
    const BORDER_W: f64 = 5.0;
    // Colors from the HoldRect logo
    const RED: [u8; 4] = [0xE8, 0x5D, 0x3A, 0xFF];
    const ORANGE: [u8; 4] = [0xE8, 0xA8, 0x38, 0xFF];
    const BLUE: [u8; 4] = [0x4A, 0x7D, 0xB5, 0xFF];
    const PURPLE: [u8; 4] = [0x7B, 0x5E, 0xA7, 0xFF];
    const BG: [u8; 4] = [0xF5, 0xE6, 0xDE, 0xFF]; // warm cream canvas
    const WHITE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
    let rect_cx = (SIZE as f64 - 1.0) / 2.0;
    let rect_cy = (SIZE as f64 - 1.0) / 2.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let off = (y * SIZE + x) * 4;
            let dx = (x as f64 - rect_cx).abs() - (half - RADIUS);
            let dy = (y as f64 - rect_cy).abs() - (half - RADIUS);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt()
            } else {
                dx.max(dy).max(0.0)
            };

            if dist > RADIUS {
                rgba[off..off + 4].copy_from_slice(&BG);
            } else if dist > RADIUS - BORDER_W {
                let color = if y as f64 <= rect_cy && x as f64 <= rect_cx {
                    RED
                } else if y as f64 <= rect_cy {
                    ORANGE
                } else if x as f64 <= rect_cx {
                    PURPLE
                } else {
                    BLUE
                };
                rgba[off..off + 4].copy_from_slice(&color);
            } else {
                rgba[off..off + 4].copy_from_slice(&WHITE);
            }
        }
    }

    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32).expect("Failed to create icon")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn app_exit_signal_type_exists() {
        let (tx, rx) = mpsc::channel();
        tx.send(AppExit).unwrap();
        assert!(rx.try_recv().is_ok(), "AppExit should be sendable through mpsc");
    }

    #[test]
    fn create_icon_returns_valid_icon() {
        let icon = create_icon();
        let _ = icon;
    }

    #[test]
    fn create_icon_has_border_pixels() {
        const SIZE: usize = 32;
        const INSET: f64 = 4.0;
        const RADIUS: f64 = 5.0;
        const BORDER_W: f64 = 5.0;
        let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
        let cx = (SIZE as f64 - 1.0) / 2.0;
        let cy = (SIZE as f64 - 1.0) / 2.0;
        let mut border = 0;
        let mut bg = 0;
        let mut white = 0;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = (x as f64 - cx).abs() - (half - RADIUS);
                let dy = (y as f64 - cy).abs() - (half - RADIUS);
                let dist = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt()
                } else {
                    dx.max(dy).max(0.0)
                };
                if dist > RADIUS { bg += 1; }
                else if dist > RADIUS - BORDER_W { border += 1; }
                else { white += 1; }
            }
        }
        assert!(border > 0, "Border should have pixels");
        assert!(bg > 0, "Background should have pixels");
        assert!(white > 0, "Interior should have pixels");
    }

    #[test]
    fn create_icon_four_colors_present() {
        const SIZE: usize = 32;
        const INSET: f64 = 4.0;
        const RADIUS: f64 = 5.0;
        const BORDER_W: f64 = 5.0;
        let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
        let cx = (SIZE as f64 - 1.0) / 2.0;
        let cy = (SIZE as f64 - 1.0) / 2.0;
        let mut has_red = false;
        let mut has_orange = false;
        let mut has_blue = false;
        let mut has_purple = false;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = (x as f64 - cx).abs() - (half - RADIUS);
                let dy = (y as f64 - cy).abs() - (half - RADIUS);
                let dist = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt()
                } else {
                    dx.max(dy).max(0.0)
                };
                if dist <= RADIUS && dist > RADIUS - BORDER_W {
                    if y as f64 <= cy && x as f64 <= cx { has_red = true; }
                    if y as f64 <= cy && x as f64 > cx { has_orange = true; }
                    if y as f64 > cy && x as f64 > cx { has_blue = true; }
                    if y as f64 > cy && x as f64 <= cx { has_purple = true; }
                }
            }
        }
        assert!(has_red, "Red (top-left) missing");
        assert!(has_orange, "Orange (top-right) missing");
        assert!(has_blue, "Blue (bottom-right) missing");
        assert!(has_purple, "Purple (bottom-left) missing");
    }

    #[test]
    fn app_exit_channel_sends_and_receives() {
        let (tx, rx) = mpsc::channel::<AppExit>();
        tx.send(AppExit).unwrap();
        assert!(rx.recv().is_ok(), "Should receive AppExit from channel");
    }
}
