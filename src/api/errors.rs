use std::env::VarError;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub struct BF1ApiError {
    api_function: Option<String>,
    source: BF1ApiSubError,
}

pub trait ApiResultExt<T> {
    fn provide_api_function(self, api_function: &str) -> Result<T, BF1ApiError>;
}

impl<T> ApiResultExt<T> for Result<T, BF1ApiError> {
    fn provide_api_function(self, api_function: &str) -> Result<T, BF1ApiError> {
        self.map_err(|err| BF1ApiError {
            api_function: Some(api_function.to_string()),
            source: err.source,
        })
    }
}

impl From<BF1ApiSubError> for BF1ApiError {
    fn from(err: BF1ApiSubError) -> Self {
        Self {
            api_function: None,
            source: err,
        }
    }
}

impl Display for BF1ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(api_function) = &self.api_function {
            return write!(
                f,
                "[BF1APIError] {}: {}",
                api_function,
                self.source.to_string()
            );
        }
        write!(f, "{}", self.source.to_string())
    }
}

impl Error for BF1ApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug)]
pub enum BF1ApiSubError {
    RequestError(reqwest::Error),
    ResponseError(String),
    JsonError(String),
    VarError { var: String, err: VarError },
    EnvError(String),
}

impl Display for BF1ApiSubError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BF1ApiSubError::RequestError(err) => {
                write!(f, "{}", err.to_string())
            }
            BF1ApiSubError::ResponseError(err) => {
                write!(f, "{}", err.to_string())
            }
            BF1ApiSubError::VarError { var, err } => {
                write!(f, "ENV error for VAR {}, message: {}", var, err.to_string())
            }
            BF1ApiSubError::EnvError(err) => {
                write!(f, "ENV error: {}", err.to_string())
            }
            BF1ApiSubError::JsonError(err) => {
                write!(f, "Json Error: {}", err)
            }
        }
    }
}

impl Error for BF1ApiSubError {}

impl From<reqwest::Error> for BF1ApiError {
    fn from(err: reqwest::Error) -> Self {
        BF1ApiError::from(BF1ApiSubError::RequestError(err))
    }
}

impl From<serde_json::Error> for BF1ApiError {
    fn from(err: serde_json::Error) -> Self {
        BF1ApiError::from(BF1ApiSubError::JsonError(err.to_string()))
    }
}
