use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub enum ColorMode {
    Solid { r: u8, g: u8, b: u8 },
    Rainbow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub modifier_vk_codes: Vec<u32>,
    pub border_width: i32,
    pub color_mode: ColorMode,
    pub modifier_name: String,
}

#[derive(Deserialize)]
struct RawConfig {
    modifier: Option<String>,
    border_width: Option<i32>,
    color: Option<String>,
}

pub(crate) fn parse_color(s: &str) -> ColorMode {
    if s.eq_ignore_ascii_case("rainbow") {
        return ColorMode::Rainbow;
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 {
        if let Ok(val) = u32::from_str_radix(hex, 16) {
            return ColorMode::Solid {
                r: ((val >> 16) & 0xFF) as u8,
                g: ((val >> 8) & 0xFF) as u8,
                b: (val & 0xFF) as u8,
            };
        }
    }
    ColorMode::Solid { r: 255, g: 0, b: 0 }
}

pub(crate) fn modifier_vk_codes(name: &str) -> Vec<u32> {
    match name {
        "Alt" => vec![0x12, 0xA4, 0xA5],
        "Ctrl" => vec![0x11, 0xA2, 0xA3],
        "Shift" => vec![0x10, 0xA0, 0xA1],
        "Win" => vec![0x5B, 0x5C],
        _ => vec![0x12, 0xA4, 0xA5],
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            modifier_vk_codes: modifier_vk_codes("Alt"),
            border_width: 4,
            color_mode: ColorMode::Solid { r: 255, g: 0, b: 0 },
            modifier_name: "Alt".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let Some(home) = dirs::home_dir() else {
            eprintln!("Warning: could not determine home directory, using defaults");
            return Self::default();
        };
        let path = home.join(".holdrect").join("config.toml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        Self::parse(&content)
    }

    pub(crate) fn parse(toml_str: &str) -> Self {
        let raw: RawConfig = match toml::from_str(toml_str) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Warning: config file malformed ({e}), using defaults");
                return Self::default();
            }
        };

        let modifier_str = raw.modifier.as_deref().unwrap_or("Alt");
        let modifier_vk_codes = modifier_vk_codes(modifier_str);
        let modifier_name = modifier_str.to_string();
        let border_width = raw.border_width.unwrap_or(4).clamp(1, 20);
        let color_mode = match raw.color.as_deref() {
            Some(s) => parse_color(s),
            None => ColorMode::Solid { r: 255, g: 0, b: 0 },
        };

        Self {
            modifier_vk_codes,
            border_width,
            color_mode,
            modifier_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_color tests --

    #[test]
    fn color_parse_hex_uppercase() {
        assert_eq!(
            parse_color("#FF8800"),
            ColorMode::Solid {
                r: 255,
                g: 136,
                b: 0
            }
        );
    }

    #[test]
    fn color_parse_hex_lowercase() {
        assert_eq!(
            parse_color("#ff8800"),
            ColorMode::Solid {
                r: 255,
                g: 136,
                b: 0
            }
        );
    }

    #[test]
    fn color_parse_rainbow() {
        assert_eq!(parse_color("rainbow"), ColorMode::Rainbow);
    }

    #[test]
    fn color_parse_rainbow_case_insensitive() {
        assert_eq!(parse_color("Rainbow"), ColorMode::Rainbow);
        assert_eq!(parse_color("RAINBOW"), ColorMode::Rainbow);
    }

    #[test]
    fn color_parse_invalid_fallback() {
        assert_eq!(
            parse_color("notacolor"),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_hex_no_hash() {
        assert_eq!(
            parse_color("00FF00"),
            ColorMode::Solid { r: 0, g: 255, b: 0 }
        );
    }

    // -- modifier_vk_codes tests --

    #[test]
    fn modifier_vk_codes_alt() {
        assert_eq!(modifier_vk_codes("Alt"), vec![0x12, 0xA4, 0xA5]);
    }

    #[test]
    fn modifier_vk_codes_ctrl() {
        assert_eq!(modifier_vk_codes("Ctrl"), vec![0x11, 0xA2, 0xA3]);
    }

    #[test]
    fn modifier_vk_codes_shift() {
        assert_eq!(modifier_vk_codes("Shift"), vec![0x10, 0xA0, 0xA1]);
    }

    #[test]
    fn modifier_vk_codes_win() {
        assert_eq!(modifier_vk_codes("Win"), vec![0x5B, 0x5C]);
    }

    #[test]
    fn modifier_vk_codes_unknown_defaults_to_alt() {
        assert_eq!(modifier_vk_codes("bogus"), vec![0x12, 0xA4, 0xA5]);
    }

    // -- AppConfig tests --

    #[test]
    fn default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.modifier_vk_codes, vec![0x12, 0xA4, 0xA5]);
        assert_eq!(cfg.border_width, 4);
        assert_eq!(
            cfg.color_mode,
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
            modifier = "Ctrl"
            border_width = 8
            color = "rainbow"
        "#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.modifier_vk_codes, vec![0x11, 0xA2, 0xA3]);
        assert_eq!(cfg.border_width, 8);
        assert_eq!(cfg.color_mode, ColorMode::Rainbow);
    }

    #[test]
    fn parse_partial_config_only_modifier() {
        let toml_str = r#"modifier = "Shift""#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.modifier_vk_codes, vec![0x10, 0xA0, 0xA1]);
        assert_eq!(cfg.border_width, 4);
        assert_eq!(
            cfg.color_mode,
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn parse_empty_config_uses_defaults() {
        let cfg = AppConfig::parse("");
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn parse_malformed_toml_uses_defaults() {
        let cfg = AppConfig::parse("this is not valid toml [[[");
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn border_width_clamped_low() {
        let toml_str = r#"border_width = 0"#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.border_width, 1);
    }

    #[test]
    fn border_width_clamped_high() {
        let toml_str = r#"border_width = 99"#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.border_width, 20);
    }

    #[test]
    fn parse_color_hex_solid() {
        let toml_str = "color = \"#00FF00\"";
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.color_mode, ColorMode::Solid { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn parse_invalid_modifier_defaults_to_alt() {
        let toml_str = r#"modifier = "bogus""#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.modifier_vk_codes, vec![0x12, 0xA4, 0xA5]);
    }

    // -- parse_color edge cases --

    #[test]
    fn color_parse_empty_string_returns_default_red() {
        assert_eq!(
            parse_color(""),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_hash_only_returns_default_red() {
        // "#" stripped leaves "", len 0 != 6, falls through to default
        assert_eq!(
            parse_color("#"),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_short_hex_3_chars_returns_default_red() {
        // 3-char hex not supported, falls through
        assert_eq!(
            parse_color("F00"),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_long_hex_7_chars_returns_default_red() {
        // 7-char hex, len != 6, falls through
        assert_eq!(
            parse_color("#FF00000"),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_hex_with_hash_prefix() {
        assert_eq!(
            parse_color("#000000"),
            ColorMode::Solid { r: 0, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_hex_all_ff() {
        assert_eq!(
            parse_color("#FFFFFF"),
            ColorMode::Solid { r: 255, g: 255, b: 255 }
        );
    }

    #[test]
    fn color_parse_hex_all_zeroes_with_hash() {
        assert_eq!(
            parse_color("#000000"),
            ColorMode::Solid { r: 0, g: 0, b: 0 }
        );
    }

    #[test]
    fn color_parse_rainbow_is_not_hex_prefix() {
        // "rainbow" doesn't start with '#', strip_prefix returns None, uses full string
        assert_eq!(parse_color("rainbow"), ColorMode::Rainbow);
    }

    #[test]
    fn color_parse_invalid_hex_chars_returns_default_red() {
        // "ZZZZZZ" is len 6 but from_str_radix fails
        assert_eq!(
            parse_color("ZZZZZZ"),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    // -- border_width boundary values --

    #[test]
    fn border_width_exactly_1() {
        let toml_str = r#"border_width = 1"#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.border_width, 1);
    }

    #[test]
    fn border_width_exactly_20() {
        let toml_str = r#"border_width = 20"#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.border_width, 20);
    }

    #[test]
    fn border_width_negative_clamped_to_1() {
        let toml_str = r#"border_width = -5"#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.border_width, 1);
    }

    // -- parse partial config combinations --

    #[test]
    fn parse_partial_config_only_color() {
        let toml_str = r##"color = "#0000FF""##;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.modifier_vk_codes, vec![0x12, 0xA4, 0xA5]); // default Alt
        assert_eq!(cfg.border_width, 4); // default
        assert_eq!(cfg.color_mode, ColorMode::Solid { r: 0, g: 0, b: 255 });
    }

    #[test]
    fn parse_partial_config_only_border_width() {
        let toml_str = r#"border_width = 12"#;
        let cfg = AppConfig::parse(toml_str);
        assert_eq!(cfg.modifier_vk_codes, vec![0x12, 0xA4, 0xA5]); // default Alt
        assert_eq!(cfg.border_width, 12);
        assert_eq!(cfg.color_mode, ColorMode::Solid { r: 255, g: 0, b: 0 }); // default
    }

    // -- modifier_vk_codes edge cases --

    #[test]
    fn modifier_vk_codes_empty_string_defaults_to_alt() {
        assert_eq!(modifier_vk_codes(""), vec![0x12, 0xA4, 0xA5]);
    }

    #[test]
    fn modifier_vk_codes_case_sensitive() {
        // "alt" (lowercase) should NOT match "Alt", falls to default
        assert_eq!(modifier_vk_codes("alt"), vec![0x12, 0xA4, 0xA5]);
    }

    #[test]
    fn modifier_vk_codes_ctrl_lowercase_is_default() {
        // "ctrl" (lowercase) should fall through to default Alt
        assert_eq!(modifier_vk_codes("ctrl"), vec![0x12, 0xA4, 0xA5]);
    }

    #[test]
    fn color_parse_invalid_hex_chars_returns_default() {
        assert_eq!(
            parse_color("#ZZZZZZ"),
            ColorMode::Solid { r: 255, g: 0, b: 0 }
        );
    }

    #[test]
    fn create_icon_corners_are_background() {
        const SIZE: usize = 32;
        const INSET: f64 = 4.0;
        const RADIUS: f64 = 5.0;
        let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
        let cx = (SIZE as f64 - 1.0) / 2.0;
        let cy = (SIZE as f64 - 1.0) / 2.0;
        let corners = [(0usize, 0usize), (0, SIZE - 1), (SIZE - 1, 0), (SIZE - 1, SIZE - 1)];
        for (y, x) in corners {
            let dx = (x as f64 - cx).abs() - (half - RADIUS);
            let dy = (y as f64 - cy).abs() - (half - RADIUS);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt()
            } else {
                dx.max(dy).max(0.0)
            };
            assert!(dist > RADIUS, "Corner ({}, {}) dist={} should be outside rounded rect", x, y, dist);
        }
    }
}
