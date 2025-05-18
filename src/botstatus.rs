use crossterm::style::Color;
use std::fmt::{Display, Formatter};
use std::time::Instant;

#[derive(Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub enum StatusTypes {
    Online,
    Crashed,
    Disabled,
    WaitingForNewMap,
    WaitingForBF1,
}

impl Display for StatusTypes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusTypes::Online => write!(f, "Online"),
            StatusTypes::Crashed => write!(f, "Crashed"),
            StatusTypes::Disabled => write!(f, "Disabled (Player Count Too Low)"),
            StatusTypes::WaitingForNewMap => write!(f, "Waiting for new map"),
            StatusTypes::WaitingForBF1 => write!(f, "Waiting for BF1 window"),
        }
    }
}

impl From<StatusTypes> for Color {
    fn from(value: StatusTypes) -> Self {
        match value {
            StatusTypes::Online => Color::Green,
            StatusTypes::WaitingForNewMap => Color::DarkGreen,
            StatusTypes::Crashed => Color::Red,
            StatusTypes::Disabled => Color::DarkYellow,
            StatusTypes::WaitingForBF1 => Color::Rgb {
                r: 255,
                g: 165,
                b: 0,
            },
        }
    }
}

#[derive(Clone)]
pub struct BotStatus {
    pub status: StatusTypes,
    pub timer_start: Instant,
    pub map_start: String,
    pub last_valid_name: Option<String>,
}
