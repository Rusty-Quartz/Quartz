use std::fmt::{self, Formatter};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(unix)]
use termion::{color, style};

/// Highest level definition of a chat color which can either be predefined or custom as of minecraft
/// 1.16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum Color {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
    Reset,
    /// A custom RBG color.
    Custom(u8, u8, u8),
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let string = match self {
            Color::Black => "black",
            Color::DarkBlue => "dark_blue",
            Color::DarkGreen => "dark_green",
            Color::DarkAqua => "dark_aqua",
            Color::DarkRed => "dark_red",
            Color::DarkPurple => "dark_purple",
            Color::Gold => "gold",
            Color::Gray => "gray",
            Color::DarkGray => "dark_gray",
            Color::Blue => "blue",
            Color::Green => "green",
            Color::Aqua => "aqua",
            Color::Red => "red",
            Color::LightPurple => "light_purple",
            Color::Yellow => "yellow",
            Color::White => "white",
            Color::Reset => "reset",
            &Color::Custom(r, g, b) => {
                return serializer.serialize_str(&format!(
                    "#{:06X}",
                    (r as u32) << 16 | (g as u32) << 8 | (b as u32)
                ));
            }
        };

        serializer.serialize_str(string)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        let string: &'de str = Deserialize::deserialize(deserializer)?;

        match string {
            "black" => Ok(Color::Black),
            "dark_blue" => Ok(Color::DarkBlue),
            "dark_green" => Ok(Color::DarkGreen),
            "dark_aqua" => Ok(Color::DarkAqua),
            "dark_red" => Ok(Color::DarkRed),
            "dark_purple" => Ok(Color::DarkPurple),
            "gold" => Ok(Color::Gold),
            "gray" => Ok(Color::Gray),
            "dark_gray" => Ok(Color::DarkGray),
            "blue" => Ok(Color::Blue),
            "green" => Ok(Color::Green),
            "aqua" => Ok(Color::Aqua),
            "red" => Ok(Color::Red),
            "light_purple" => Ok(Color::LightPurple),
            "yellow" => Ok(Color::Yellow),
            "white" => Ok(Color::White),
            "reset" => Ok(Color::Reset),
            _ => {
                if string.is_empty() {
                    return Err(de::Error::custom(
                        "Expected hex color, found an empty string.",
                    ));
                }

                if string.len() != 7 {
                    return Err(de::Error::custom(
                        "Expected hex color in the form of '#RRGGBB'",
                    ));
                }

                if let Ok(rgb) = u32::from_str_radix(&string[1 ..], 16) {
                    Ok(Color::Custom(
                        (rgb >> 16) as u8,
                        (rgb >> 8) as u8,
                        rgb as u8,
                    ))
                } else {
                    Err(de::Error::custom(
                        "Invalid hex color, expected 6 hexadecimal digits (0-F).",
                    ))
                }
            }
        }
    }
}

impl Color {
    /// Applies the color to the terminal (unix only).
    #[cfg(unix)]
    pub fn apply(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Color::Black => write!(f, "{}", color::Fg(color::Black)),
            Color::DarkBlue => write!(f, "{}", color::Fg(color::Blue)),
            Color::DarkGreen => write!(f, "{}", color::Fg(color::Green)),
            Color::DarkAqua => write!(f, "{}", color::Fg(color::Cyan)),
            Color::DarkRed => write!(f, "{}", color::Fg(color::Red)),
            Color::DarkPurple => write!(f, "{}", color::Fg(color::Magenta)),
            Color::Gold => write!(f, "{}", color::Fg(color::Yellow)),
            Color::Gray => write!(f, "{}", color::Fg(color::White)),
            Color::DarkGray => write!(f, "{}", color::Fg(color::LightBlack)),
            Color::Blue => write!(f, "{}", color::Fg(color::LightBlue)),
            Color::Green => write!(f, "{}", color::Fg(color::LightGreen)),
            Color::Aqua => write!(f, "{}", color::Fg(color::LightCyan)),
            Color::Red => write!(f, "{}", color::Fg(color::LightRed)),
            Color::LightPurple => write!(f, "{}", color::Fg(color::LightMagenta)),
            Color::Yellow => write!(f, "{}", color::Fg(color::LightYellow)),
            Color::White => write!(f, "{}", color::Fg(color::LightWhite)),
            Color::Reset => write!(f, "{}{}", color::Fg(color::Reset), style::Reset),
            // Dividing by 43 maps the color to the correct ANSI range of [0,5]
            &Color::Custom(r, g, b) => write!(
                f,
                "{}",
                color::Fg(color::AnsiValue::rgb(r / 43, g / 43, b / 43))
            ),
        }
    }
}
