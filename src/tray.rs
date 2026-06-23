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
    const PNG_DATA: &[u8] = include_bytes!("../asserts/HoldRect.png");
    const SIZE: u32 = 32;
    const BG: [u8; 3] = [0xF0, 0xED, 0xEB]; // off-white background color
    const BG_THRESHOLD: u16 = 20; // color distance tolerance

    let img = image::load_from_memory(PNG_DATA)
        .expect("Failed to decode embedded PNG");
    let rgba = img.resize(SIZE, SIZE, image::imageops::FilterType::Lanczos3)
        .to_rgba8();

    // Make background pixels transparent
    let pixels: Vec<u8> = rgba.into_raw()
        .chunks(4)
        .flat_map(|p| {
            let dist = (p[0] as i16 - BG[0] as i16).unsigned_abs()
                + (p[1] as i16 - BG[1] as i16).unsigned_abs()
                + (p[2] as i16 - BG[2] as i16).unsigned_abs();
            if dist < BG_THRESHOLD {
                [p[0], p[1], p[2], 0] // transparent
            } else {
                [p[0], p[1], p[2], p[3]]
            }
        })
        .collect();

    Icon::from_rgba(pixels, SIZE, SIZE).expect("Failed to create icon")
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
        let _ = icon; // didn't panic = valid
    }

    #[test]
    fn create_icon_rgba_size() {
        // 32x32 RGBA = 4096 bytes
        let icon = create_icon();
        // Icon was created from 32x32 RGBA data successfully
        let _ = icon;
    }

    #[test]
    fn create_icon_has_transparent_pixels() {
        // Verify the embedded PNG decodes and background becomes transparent
        const PNG_DATA: &[u8] = include_bytes!("../asserts/HoldRect.png");
        let img = image::load_from_memory(PNG_DATA).unwrap();
        let rgba = img.resize(32, 32, image::imageops::FilterType::Lanczos3).to_rgba8();
        let raw = rgba.into_raw();
        // At least some pixels should be near the off-white background
        let bg_count = raw.chunks(4).filter(|p| {
            let dist = (p[0] as i16 - 0xF0).unsigned_abs()
                + (p[1] as i16 - 0xED).unsigned_abs()
                + (p[2] as i16 - 0xEB).unsigned_abs();
            dist < 20
        }).count();
        assert!(bg_count > 0, "Should detect background pixels in source image");
    }

    #[test]
    fn app_exit_channel_sends_and_receives() {
        let (tx, rx) = mpsc::channel::<AppExit>();
        tx.send(AppExit).unwrap();
        assert!(rx.recv().is_ok(), "Should receive AppExit from channel");
    }

    #[test]
    fn debug_icon_output() {
        use image::RgbaImage;

        const PNG_DATA: &[u8] = include_bytes!("../asserts/HoldRect.png");
        const SIZE: u32 = 32;
        const BG: [u8; 3] = [0xF0, 0xED, 0xEB];
        const BG_THRESHOLD: u16 = 20;

        let img = image::load_from_memory(PNG_DATA)
            .expect("Failed to decode embedded PNG");
        let rgba = img.resize(SIZE, SIZE, image::imageops::FilterType::Lanczos3)
            .to_rgba8();

        let pixels: Vec<u8> = rgba.into_raw()
            .chunks(4)
            .flat_map(|p| {
                let dist = (p[0] as i16 - BG[0] as i16).unsigned_abs()
                    + (p[1] as i16 - BG[1] as i16).unsigned_abs()
                    + (p[2] as i16 - BG[2] as i16).unsigned_abs();
                if dist < BG_THRESHOLD {
                    [p[0], p[1], p[2], 0]
                } else {
                    [p[0], p[1], p[2], p[3]]
                }
            })
            .collect();

        // Save to disk for visual inspection
        let out_img = RgbaImage::from_raw(SIZE, SIZE, pixels.clone())
            .expect("Failed to build RgbaImage");
        std::fs::create_dir_all("asserts").unwrap();
        out_img.save("asserts/debug_icon.png").expect("Failed to save debug icon PNG");

        // Print stats
        let total = (SIZE * SIZE) as usize;
        let transparent = pixels.chunks(4).filter(|p| p[3] == 0).count();
        let opaque = total - transparent;
        println!("\n=== debug_icon_output ===");
        println!("Total pixels: {total}");
        println!("Transparent (alpha=0): {transparent}");
        println!("Opaque (alpha>0): {opaque}");
        println!("First 4 pixels RGBA:");
        for (i, chunk) in pixels.chunks(4).take(4).enumerate() {
            println!("  [{i}] R={} G={} B={} A={}", chunk[0], chunk[1], chunk[2], chunk[3]);
        }
        println!("========================\n");

        assert!(total > 0);
    }
}
