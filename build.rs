fn main() {
    // Generate .ico file from the same icon logic used in tray.rs
    let ico_path = generate_ico();

    // Embed the .ico as the exe's Windows resource (IDI_ICON = 1)
    if cfg!(target_os = "windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&ico_path.to_string_lossy());
        res.compile().expect("Failed to compile Windows resource");
    }
}

fn generate_ico() -> std::path::PathBuf {
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

    // Generate RGBA pixels (same algorithm as src/tray.rs create_icon)
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

            let color = if dist > RADIUS {
                BG
            } else if dist > RADIUS - BORDER_W {
                if y as f64 <= rect_cy && x as f64 <= rect_cx {
                    RED
                } else if y as f64 <= rect_cy {
                    ORANGE
                } else if x as f64 <= rect_cx {
                    PURPLE
                } else {
                    BLUE
                }
            } else {
                WHITE
            };
            rgba[off..off + 4].copy_from_slice(&color);
        }
    }

    // Convert RGBA → BGRA, flip vertically (BMP is bottom-up)
    let mut bgra = vec![0u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        let src_row = y;
        let dst_row = SIZE - 1 - y;
        for x in 0..SIZE {
            let s = (src_row * SIZE + x) * 4;
            let d = (dst_row * SIZE + x) * 4;
            bgra[d] = rgba[s + 2];     // B
            bgra[d + 1] = rgba[s + 1]; // G
            bgra[d + 2] = rgba[s];     // R
            bgra[d + 3] = rgba[s + 3]; // A
        }
    }

    // AND mask: all zeros (alpha channel handles transparency)
    let and_mask = vec![0u8; SIZE * (SIZE / 8)];

    // Build .ico file
    let data_offset: u32 = 6 + 16; // ICONDIR + 1 ICONDIRENTRY
    let image_size: u32 = 40 + (SIZE * SIZE * 4) as u32 + and_mask.len() as u32; // BITMAPINFOHEADER + XOR + AND

    let mut ico = Vec::new();
    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type = ICO
    ico.extend_from_slice(&1u16.to_le_bytes()); // count = 1
    // ICONDIRENTRY
    ico.push(SIZE as u8); // width
    ico.push(SIZE as u8); // height
    ico.push(0); // color count (0 for 32bpp)
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&image_size.to_le_bytes());
    ico.extend_from_slice(&data_offset.to_le_bytes());
    // BITMAPINFOHEADER
    ico.extend_from_slice(&40u32.to_le_bytes()); // header size
    ico.extend_from_slice(&(SIZE as i32).to_le_bytes()); // width
    ico.extend_from_slice(&((SIZE * 2) as i32).to_le_bytes()); // height (doubled for ICO)
    ico.extend_from_slice(&1u16.to_le_bytes()); // planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
    ico.extend_from_slice(&0u32.to_le_bytes()); // compression
    ico.extend_from_slice(&(SIZE as u32 * SIZE as u32 * 4).to_le_bytes()); // image size
    ico.extend_from_slice(&[0u8; 16]); // rest of header (zeroed)
    // XOR mask (BGRA pixels)
    ico.extend_from_slice(&bgra);
    // AND mask
    ico.extend_from_slice(&and_mask);

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico_path = out_dir.join("holdrect.ico");
    std::fs::write(&ico_path, &ico).expect("Failed to write .ico file");
    ico_path
}
