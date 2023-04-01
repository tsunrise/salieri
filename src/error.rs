use serde_json::json;
use thiserror::Error;
use worker::Response;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("internal error")]
    WorkerError(#[from] worker::Error),
}

impl Error {
    fn status_code(&self) -> u16 {
        match self {
            Error::InvalidRequest(_) => 400,
            Error::WorkerError(_) => 500,
        }
    }

    fn json(&self) -> serde_json::Value {
        json!({
            "error": self.to_string(),
            "status_code": self.status_code(),
        })
    }
}

impl From<Error> for Response {
    fn from(err: Error) -> Self {
        let error_code = err.status_code();
        let resp = Response::from_json(&err.json())
            .unwrap()
            .with_status(error_code);
        resp
    }
}

pub type Result<T> = std::result::Result<T, Error>;
