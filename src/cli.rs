use std::fmt;
use std::sync::mpsc::Sender;

pub const DEFAULT_ZOOM: f64 = 2.0;
pub const MIN_ZOOM: f64 = 1.5;
pub const MAX_ZOOM: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Rect {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },
    Magnifier {
        x: i32,
        y: i32,
        zoom: f64,
    },
    Clear,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StartupMode {
    Resident { first_launch: bool },
    Client(CliCommand),
    MemoryReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn is_code(&self, code: &str) -> bool {
        self.code == code
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

pub struct CommandEnvelope {
    pub command: CliCommand,
    pub reply_tx: Sender<Result<(), CommandError>>,
}

pub fn parse_startup_args(args: &[String]) -> Result<StartupMode, CommandError> {
    if args.is_empty() {
        return Ok(StartupMode::Resident { first_launch: true });
    }
    if args.len() == 1 && args[0] == "--daemon" {
        return Ok(StartupMode::Resident { first_launch: false });
    }
    if args.len() == 1 && args[0] == "--mem-report" {
        return Ok(StartupMode::MemoryReport);
    }

    parse_visual_command(args).map(StartupMode::Client)
}

fn parse_visual_command(args: &[String]) -> Result<CliCommand, CommandError> {
    match args.first().map(String::as_str) {
        Some("rect") if args.len() == 5 => {
            let x1 = parse_i32(&args[1], "x1")?;
            let y1 = parse_i32(&args[2], "y1")?;
            let x2 = parse_i32(&args[3], "x2")?;
            let y2 = parse_i32(&args[4], "y2")?;
            if x1 == x2 || y1 == y2 {
                return Err(CommandError::new(
                    "invalid_rect",
                    "rectangle width and height must be non-zero",
                ));
            }
            Ok(CliCommand::Rect { x1, y1, x2, y2 })
        }
        Some("magnifier") if args.len() == 3 || args.len() == 4 => {
            let x = parse_i32(&args[1], "x")?;
            let y = parse_i32(&args[2], "y")?;
            let zoom = if args.len() == 4 {
                args[3]
                    .parse::<f64>()
                    .map_err(|_| CommandError::new("invalid_zoom", "zoom must be a number"))?
            } else {
                DEFAULT_ZOOM
            };
            if !zoom.is_finite() || !(MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
                return Err(CommandError::new(
                    "invalid_zoom",
                    format!("zoom must be between {MIN_ZOOM} and {MAX_ZOOM}"),
                ));
            }
            Ok(CliCommand::Magnifier { x, y, zoom })
        }
        Some("clear") if args.len() == 1 => Ok(CliCommand::Clear),
        _ => Err(CommandError::new(
            "usage",
            "usage: holdrect rect x1 y1 x2 y2 | magnifier x y [zoom] | clear",
        )),
    }
}

fn parse_i32(value: &str, name: &str) -> Result<i32, CommandError> {
    value.parse::<i32>().map_err(|_| {
        CommandError::new(
            "invalid_coordinate",
            format!("{name} must be a signed 32-bit integer"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn no_arguments_select_normal_resident_mode() {
        assert_eq!(
            parse_startup_args(&[]).unwrap(),
            StartupMode::Resident { first_launch: true }
        );
    }

    #[test]
    fn daemon_selects_silent_resident_mode() {
        assert_eq!(
            parse_startup_args(&args(&["--daemon"])).unwrap(),
            StartupMode::Resident { first_launch: false }
        );
    }

    #[test]
    fn mem_report_remains_immediate_mode() {
        assert_eq!(
            parse_startup_args(&args(&["--mem-report"])).unwrap(),
            StartupMode::MemoryReport
        );
    }

    #[test]
    fn rect_accepts_signed_reversed_corners() {
        assert_eq!(
            parse_startup_args(&args(&["rect", "500", "-20", "100", "400"])).unwrap(),
            StartupMode::Client(CliCommand::Rect {
                x1: 500,
                y1: -20,
                x2: 100,
                y2: 400,
            })
        );
    }

    #[test]
    fn magnifier_uses_default_zoom() {
        assert_eq!(
            parse_startup_args(&args(&["magnifier", "800", "450"])).unwrap(),
            StartupMode::Client(CliCommand::Magnifier {
                x: 800,
                y: 450,
                zoom: 2.0,
            })
        );
    }

    #[test]
    fn magnifier_accepts_zoom_boundaries() {
        for zoom in ["1.5", "8"] {
            assert!(parse_startup_args(&args(&["magnifier", "0", "0", zoom])).is_ok());
        }
    }

    #[test]
    fn clear_has_no_arguments() {
        assert_eq!(
            parse_startup_args(&args(&["clear"])).unwrap(),
            StartupMode::Client(CliCommand::Clear)
        );
    }

    #[test]
    fn invalid_commands_are_rejected() {
        let cases = [
            args(&["rect", "0", "0", "0", "10"]),
            args(&["rect", "0", "0", "10", "0"]),
            args(&["rect", "x", "0", "10", "10"]),
            args(&["rect", "0", "0", "2147483648", "10"]),
            args(&["magnifier", "0", "0", "1.49"]),
            args(&["magnifier", "0", "0", "8.01"]),
            args(&["magnifier", "0", "0", "NaN"]),
            args(&["magnifier", "0", "0", "inf"]),
            args(&["clear", "extra"]),
            args(&["unknown"]),
            args(&["--daemon", "extra"]),
            args(&["--mem-report", "extra"]),
        ];

        for case in cases {
            assert!(parse_startup_args(&case).is_err(), "accepted {case:?}");
        }
    }
}
