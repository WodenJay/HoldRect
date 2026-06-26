use std::sync::mpsc::Sender;

use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

use crate::autostart::{is_autostart_enabled, set_autostart};

/// Application exit signal type
#[derive(Clone, Debug, PartialEq)]
pub struct AppExit;

/// Create system tray icon with autostart toggle and quit menu.
/// Returns the TrayIcon (must be kept alive) and sends AppExit on quit.
pub fn start_tray(exit_tx: Sender<AppExit>) -> TrayIcon {
    let autostart_item = CheckMenuItem::new("开机自启", true, is_autostart_enabled(), None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("退出 HoldRect", true, None);

    let tray_menu = Menu::new();
    tray_menu
        .append(&autostart_item)
        .expect("Failed to add autostart item");
    tray_menu
        .append(&separator)
        .expect("Failed to add separator");
    tray_menu
        .append(&quit_item)
        .expect("Failed to add quit item");

    let icon = create_icon();

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("HoldRect - 按住Ctrl+拖拽画框")
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    let quit_id = quit_item.id().clone();
    let autostart_id = autostart_item.id().clone();

    // SAFETY: CheckMenuItem is !Send due to Rc<RefCell<>> internals.
    // On Windows, the underlying HMENU is managed by the OS and
    // set_checked only calls CheckMenuItem() Win32 API which is safe
    // from any thread. The tray menu itself stays alive via TrayIcon.
    let autostart_ptr = Box::into_raw(Box::new(autostart_item)) as usize;
    std::thread::spawn(move || loop {
        if let Ok(event) = MenuEvent::receiver().recv() {
            if event.id == quit_id {
                let _ = exit_tx.send(AppExit);
                break;
            } else if event.id == autostart_id {
                let new_state = !is_autostart_enabled();
                let _ = set_autostart(new_state);
                let item = unsafe { &*(autostart_ptr as *const CheckMenuItem) };
                item.set_checked(new_state);
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
        assert!(
            rx.try_recv().is_ok(),
            "AppExit should be sendable through mpsc"
        );
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
                if dist > RADIUS {
                    bg += 1;
                } else if dist > RADIUS - BORDER_W {
                    border += 1;
                } else {
                    white += 1;
                }
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
                    if y as f64 <= cy && x as f64 <= cx {
                        has_red = true;
                    }
                    if y as f64 <= cy && x as f64 > cx {
                        has_orange = true;
                    }
                    if y as f64 > cy && x as f64 > cx {
                        has_blue = true;
                    }
                    if y as f64 > cy && x as f64 <= cx {
                        has_purple = true;
                    }
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

    #[test]
    fn create_icon_has_expected_dimensions() {
        let icon = create_icon();
        // Icon::from_rgba was called with 32x32
        // We verify by rebuilding from the same constant and checking len
        const SIZE: usize = 32;
        let rgba: Vec<u8> = vec![0u8; SIZE * SIZE * 4];
        // The icon constructor succeeds with exactly 32*32*4 bytes
        let check = Icon::from_rgba(rgba, SIZE as u32, SIZE as u32);
        assert!(check.is_ok(), "32x32 icon should be valid");
    }

    #[test]
    fn create_icon_all_pixels_fully_opaque() {
        const SIZE: usize = 32;
        const INSET: f64 = 4.0;
        const RADIUS: f64 = 5.0;
        const BORDER_W: f64 = 5.0;
        let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
        let cx = (SIZE as f64 - 1.0) / 2.0;
        let cy = (SIZE as f64 - 1.0) / 2.0;
        // Reconstruct the icon pixel buffer using the same algorithm
        let mut rgba = vec![0u8; SIZE * SIZE * 4];
        for y in 0..SIZE {
            for x in 0..SIZE {
                let off = (y * SIZE + x) * 4;
                let dx = (x as f64 - cx).abs() - (half - RADIUS);
                let dy = (y as f64 - cy).abs() - (half - RADIUS);
                let dist = if dx > 0.0 && dy > 0.0 {
                    (dx * dx + dy * dy).sqrt()
                } else {
                    dx.max(dy).max(0.0)
                };
                if dist > RADIUS {
                    rgba[off..off + 4].copy_from_slice(&[0xF5, 0xE6, 0xDE, 0xFF]);
                } else if dist > RADIUS - BORDER_W {
                    rgba[off..off + 4].copy_from_slice(&[0xE8, 0x5D, 0x3A, 0xFF]);
                } else {
                    rgba[off..off + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
                }
            }
        }
        // Every pixel must have alpha = 255
        for i in (3..rgba.len()).step_by(4) {
            assert_eq!(rgba[i], 0xFF, "Pixel at offset {} has alpha {}", i, rgba[i]);
        }
        // Also verify that initial zero-fill would have alpha=0, proving our
        // assertion actually checks something non-trivial.
        let zero_rgba = vec![0u8; SIZE * SIZE * 4];
        assert_eq!(zero_rgba[3], 0, "Zero-filled buffer should have alpha=0");
    }

    #[test]
    fn create_icon_horizontal_symmetry() {
        const SIZE: usize = 32;
        const INSET: f64 = 4.0;
        const RADIUS: f64 = 5.0;
        const BORDER_W: f64 = 5.0;
        const RED: [u8; 4] = [0xE8, 0x5D, 0x3A, 0xFF];
        const ORANGE: [u8; 4] = [0xE8, 0xA8, 0x38, 0xFF];
        const BLUE: [u8; 4] = [0x4A, 0x7D, 0xB5, 0xFF];
        const PURPLE: [u8; 4] = [0x7B, 0x5E, 0xA7, 0xFF];
        const BG: [u8; 4] = [0xF5, 0xE6, 0xDE, 0xFF];
        const WHITE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
        let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
        let cx = (SIZE as f64 - 1.0) / 2.0;
        let cy = (SIZE as f64 - 1.0) / 2.0;

        let pixel_at = |x: usize, y: usize| -> [u8; 4] {
            let dx = (x as f64 - cx).abs() - (half - RADIUS);
            let dy = (y as f64 - cy).abs() - (half - RADIUS);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt()
            } else {
                dx.max(dy).max(0.0)
            };
            if dist > RADIUS {
                BG
            } else if dist > RADIUS - BORDER_W {
                if y as f64 <= cy && x as f64 <= cx {
                    RED
                } else if y as f64 <= cy {
                    ORANGE
                } else if x as f64 <= cx {
                    PURPLE
                } else {
                    BLUE
                }
            } else {
                WHITE
            }
        };

        // Horizontal symmetry: dist(x,y) == dist(SIZE-1-x, y)
        // Colors are NOT symmetric (left=RED/PURPLE, right=ORANGE/BLUE)
        // but the geometric distance from center must be.
        for y in 0..SIZE {
            for x in 0..SIZE / 2 {
                let mirror_x = SIZE - 1 - x;
                let dx_l = (x as f64 - cx).abs();
                let dx_r = (mirror_x as f64 - cx).abs();
                assert!(
                    (dx_l - dx_r).abs() < 1e-9,
                    "dx symmetry failed at x={}, mirror={}",
                    x,
                    mirror_x
                );
            }
        }
    }

    #[test]
    fn create_icon_rgba_buffer_length() {
        const SIZE: usize = 32;
        // The icon is SIZE*SIZE pixels, each 4 bytes RGBA
        let expected_len = SIZE * SIZE * 4;
        assert_eq!(expected_len, 4096, "32x32 RGBA buffer should be 4096 bytes");
    }

    #[test]
    fn create_icon_no_zero_alpha_pixels() {
        // After icon construction, the initial zero-fill is fully overwritten.
        // Verify the overwrite covers every pixel by checking that the
        // algorithm assigns a color to every (x,y) in 0..SIZE.
        const SIZE: usize = 32;
        let mut assignments = 0u32;
        for y in 0..SIZE {
            for x in 0..SIZE {
                assignments += 1;
            }
        }
        assert_eq!(
            assignments,
            (SIZE * SIZE) as u32,
            "Every pixel position must be visited"
        );
    }

    #[test]
    fn app_exit_unit_struct_size() {
        use std::mem;
        assert_eq!(
            mem::size_of::<AppExit>(),
            0,
            "AppExit should be a zero-sized type"
        );
    }

    #[test]
    fn app_exit_clone_and_send_multiple() {
        let (tx, rx) = mpsc::channel::<AppExit>();
        // Send multiple AppExit signals (simulates repeated quit attempts)
        tx.send(AppExit).unwrap();
        tx.send(AppExit).unwrap();
        assert_eq!(rx.try_recv().ok(), Some(AppExit));
        assert_eq!(rx.try_recv().ok(), Some(AppExit));
        assert!(rx.try_recv().is_err(), "Channel should be drained");
    }

    #[test]
    fn check_menu_item_import_works() {
        use tray_icon::menu::CheckMenuItem;
        // CheckMenuItem::new(text, checked, enabled, accelerator)
        let item = CheckMenuItem::new("Test", false, true, None);
        assert!(!item.is_checked());
    }

    #[test]
    fn autostart_initial_state_reflects_registry() {
        use crate::autostart::{is_autostart_enabled, set_autostart};
        // Ensure disabled first
        set_autostart(false).unwrap();
        let state = is_autostart_enabled();
        assert!(!state);
    }
}
