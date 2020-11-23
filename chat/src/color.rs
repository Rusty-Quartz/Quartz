use std::fmt::{self, Formatter};

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(unix)]
use termion::{color, style};

/// Highest level definition of a chat color which can either be predefined or custom as of minecraft
/// 1.16.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    /// A predefined color.
    Predefined(PredefinedColor),

    /// A custom RBG color.
    #[serde(
        serialize_with = "Color::serialize_custom",
        deserialize_with = "Color::deserialize_custom"
    )]
    Custom(
        /// The red value.
        u8,
        /// The green value.
        u8,
        /// The blue value.
        u8,
    ),
}

impl Color {
    /// Applies the color to the terminal (unix only).
    #[cfg(unix)]
    pub fn apply(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Color::Predefined(color) => color.apply(f),
            // Dividing by 43 maps the color to the correct ANSI range of [0,5]
            Color::Custom(r, g, b) => write!(
                f,
                "{}",
                color::Fg(color::AnsiValue::rgb(*r / 43, *g / 43, *b / 43))
            ),
        }
    }

    // Serde support functions for the custom color type

    fn serialize_custom<S>(r: &u8, g: &u8, b: &u8, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_str(&format!(
            "#{:06X}",
            (*r as u32) << 16 | (*g as u32) << 8 | (*b as u32)
        ))
    }

    fn deserialize_custom<'de, D>(deserializer: D) -> Result<(u8, u8, u8), D::Error>
    where D: Deserializer<'de> {
        let value: &'de str = Deserialize::deserialize(deserializer)?;

        if value.is_empty() {
            return Err(de::Error::custom(
                "Expected hex color, found an empty string.",
            ));
        }

        if value.len() != 7 {
            return Err(de::Error::custom(
                "Expected hex color in the form of '#RRGGBB'",
            ));
        }

        if let Ok(rgb) = u32::from_str_radix(&value[1 ..], 16) {
            Ok(((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
        } else {
            Err(de::Error::custom(
                "Invalid hex color, expected 6 hexadecimal digits (0-F).",
            ))
        }
    }
}

impl From<PredefinedColor> for Color {
    fn from(predef_color: PredefinedColor) -> Self {
        Color::Predefined(predef_color)
    }
}

/// All predefined color types.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum PredefinedColor {
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
}

impl PredefinedColor {
    /// Applies the color to the terminal (unix only).
    #[cfg(unix)]
    pub fn apply(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PredefinedColor::Black => write!(f, "{}", color::Fg(color::Black)),
            PredefinedColor::DarkBlue => write!(f, "{}", color::Fg(color::Blue)),
            PredefinedColor::DarkGreen => write!(f, "{}", color::Fg(color::Green)),
            PredefinedColor::DarkAqua => write!(f, "{}", color::Fg(color::Cyan)),
            PredefinedColor::DarkRed => write!(f, "{}", color::Fg(color::Red)),
            PredefinedColor::DarkPurple => write!(f, "{}", color::Fg(color::Magenta)),
            PredefinedColor::Gold => write!(f, "{}", color::Fg(color::Yellow)),
            PredefinedColor::Gray => write!(f, "{}", color::Fg(color::White)),
            PredefinedColor::DarkGray => write!(f, "{}", color::Fg(color::LightBlack)),
            PredefinedColor::Blue => write!(f, "{}", color::Fg(color::LightBlue)),
            PredefinedColor::Green => write!(f, "{}", color::Fg(color::LightGreen)),
            PredefinedColor::Aqua => write!(f, "{}", color::Fg(color::LightCyan)),
            PredefinedColor::Red => write!(f, "{}", color::Fg(color::LightRed)),
            PredefinedColor::LightPurple => write!(f, "{}", color::Fg(color::LightMagenta)),
            PredefinedColor::Yellow => write!(f, "{}", color::Fg(color::LightYellow)),
            PredefinedColor::White => write!(f, "{}", color::Fg(color::LightWhite)),
            PredefinedColor::Reset => write!(f, "{}{}", color::Fg(color::Reset), style::Reset),
        }
    }
}
