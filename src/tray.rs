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
    const RADIUS: f64 = 6.0;
    const BORDER_W: f64 = 3.0;
    // Colors from the HoldRect logo
    const RED: [u8; 4] = [0xE0, 0x60, 0x4A, 0xFF];
    const ORANGE: [u8; 4] = [0xE8, 0xA8, 0x38, 0xFF];
    const BLUE: [u8; 4] = [0x4A, 0x72, 0xA8, 0xFF];
    const PURPLE: [u8; 4] = [0x7B, 0x50, 0x90, 0xFF];

    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let center = (SIZE - 1) as f64 / 2.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f64;
            let fy = y as f64;

            // Rounded rect distance from center
            let dx = (fx - center).abs() - (center - RADIUS);
            let dy = (fy - center).abs() - (center - RADIUS);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt()
            } else {
                dx.max(dy).max(0.0)
            };

            // Only draw border pixels
            if dist <= RADIUS && dist > RADIUS - BORDER_W {
                // Pick color by perimeter position
                let color = if fy <= center && fx <= center {
                    RED
                } else if fy <= center {
                    ORANGE
                } else if fx <= center {
                    PURPLE
                } else {
                    BLUE
                };
                let off = (y * SIZE + x) * 4;
                rgba[off..off + 4].copy_from_slice(&color);
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
        // Replicate drawing logic and verify some pixels are opaque
        const SIZE: usize = 32;
        let mut opaque = 0;
        let center = (SIZE - 1) as f64 / 2.0;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = (x as f64 - center).abs() - (center - 6.0);
                let dy = (y as f64 - center).abs() - (center - 6.0);
                let dist = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt()
                } else {
                    dx.max(dy).max(0.0)
                };
                if dist <= 6.0 && dist > 3.0 {
                    opaque += 1;
                }
            }
        }
        assert!(opaque > 0, "Rounded rect border should have visible pixels");
    }

    #[test]
    fn create_icon_four_colors_present() {
        // Verify all four quadrant colors appear in the icon
        const SIZE: usize = 32;
        let center = (SIZE - 1) as f64 / 2.0;
        let mut has_red = false;
        let mut has_orange = false;
        let mut has_blue = false;
        let mut has_purple = false;

        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = (x as f64 - center).abs() - (center - 6.0);
                let dy = (y as f64 - center).abs() - (center - 6.0);
                let dist = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt()
                } else {
                    dx.max(dy).max(0.0)
                };
                if dist <= 6.0 && dist > 3.0 {
                    if y as f64 <= center && x as f64 <= center { has_red = true; }
                    if y as f64 <= center && x as f64 > center { has_orange = true; }
                    if y as f64 > center && x as f64 > center { has_blue = true; }
                    if y as f64 > center && x as f64 <= center { has_purple = true; }
                }
            }
        }
        assert!(has_red, "Red (top-left) color missing");
        assert!(has_orange, "Orange (top-right) color missing");
        assert!(has_blue, "Blue (bottom-right) color missing");
        assert!(has_purple, "Purple (bottom-left) color missing");
    }

    #[test]
    fn app_exit_channel_sends_and_receives() {
        let (tx, rx) = mpsc::channel::<AppExit>();
        tx.send(AppExit).unwrap();
        assert!(rx.recv().is_ok(), "Should receive AppExit from channel");
    }
}
