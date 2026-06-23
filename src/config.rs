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
}

#[derive(Deserialize)]
struct RawConfig {
    modifier: Option<String>,
    border_width: Option<i32>,
    color: Option<String>,
}

fn parse_color(s: &str) -> ColorMode {
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

fn modifier_vk_codes(name: &str) -> Vec<u32> {
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

    fn parse(toml_str: &str) -> Self {
        let raw: RawConfig = match toml::from_str(toml_str) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Warning: config file malformed ({e}), using defaults");
                return Self::default();
            }
        };

        let modifier_str = raw.modifier.as_deref().unwrap_or("Alt");
        let modifier_vk_codes = modifier_vk_codes(modifier_str);
        let border_width = raw.border_width.unwrap_or(4).clamp(1, 20);
        let color_mode = match raw.color.as_deref() {
            Some(s) => parse_color(s),
            None => ColorMode::Solid { r: 255, g: 0, b: 0 },
        };

        Self {
            modifier_vk_codes,
            border_width,
            color_mode,
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
}
