use crate::api::errors::BF1ApiError;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::io::ErrorKind;
use tesseract::TesseractError;

#[derive(Debug)]
pub enum KickbotError {
    DiscordError(String),
    ScreenshotError(String),
    ApiError(String),
    IOError(String),
    ModelError(String),
    TesseractError(String),
    JsonError(String),
}

impl Error for KickbotError {}

impl Display for KickbotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            KickbotError::DiscordError(ref err) => write!(f, "[Discord Error] {}", err.to_string()),
            KickbotError::ScreenshotError(ref err) => {
                write!(f, "[Screenshot Error] {}", err.to_string())
            }
            KickbotError::ApiError(ref err) => {
                write!(f, "[API Error] {}", err)
            }
            KickbotError::IOError(ref err) => {
                write!(f, "[IO Error] {}", err)
            }
            KickbotError::ModelError(ref err) => {
                write!(f, "[Model Error] {}", err)
            }
            KickbotError::TesseractError(ref err) => {
                write!(f, "[Tesseract Error] {}", err)
            }
            KickbotError::JsonError(ref err) => {
                write!(f, "[JSON Error] {}", err)
            }
        }
    }
}

impl From<BF1ApiError> for KickbotError {
    fn from(err: BF1ApiError) -> Self {
        KickbotError::ApiError(err.to_string())
    }
}

impl From<opencv::Error> for KickbotError {
    fn from(err: opencv::Error) -> Self {
        KickbotError::ScreenshotError(err.to_string())
    }
}

impl From<ort::Error> for KickbotError {
    fn from(err: ort::Error) -> Self {
        KickbotError::ModelError(err.to_string())
    }
}

impl From<TesseractError> for KickbotError {
    fn from(err: TesseractError) -> Self {
        KickbotError::TesseractError(err.to_string())
    }
}

impl From<serde_json::Error> for KickbotError {
    fn from(err: serde_json::Error) -> Self {
        KickbotError::JsonError(err.to_string())
    }
}

impl From<io::Error> for KickbotError {
    fn from(err: io::Error) -> Self {
        KickbotError::IOError(err.to_string())
    }
}

impl From<csv::Error> for KickbotError {
    fn from(err: csv::Error) -> Self {
        KickbotError::IOError(err.to_string())
    }
}

impl From<KickbotError> for io::Error {
    fn from(err: KickbotError) -> Self {
        io::Error::new(ErrorKind::Other, err)
    }
}

impl From<BF1ApiError> for io::Error {
    fn from(err: BF1ApiError) -> Self {
        io::Error::from(KickbotError::from(err))
    }
}
