use defguard_wireguard_rs::{error::WireguardInterfaceError, net::IpAddrParseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PonyError {
    #[error(transparent)]
    Database(#[from] tokio_postgres::Error),

    #[error("DB conflict")]
    Conflict,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    UrlParse(#[from] url::ParseError),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),

    #[error(transparent)]
    XrayUri(#[from] tonic::codegen::http::uri::InvalidUri),

    #[error(transparent)]
    XrayTransport(#[from] tonic::transport::Error),

    #[error(transparent)]
    Grpc(#[from] tonic::Status),

    #[error(transparent)]
    SerdeUrlEnc(#[from] serde_urlencoded::ser::Error),

    #[error(transparent)]
    Zmq(#[from] zmq::Error),

    #[error(transparent)]
    Wireguard(#[from] WireguardInterfaceError),

    #[error(transparent)]
    IpParseError(#[from] IpAddrParseError),

    #[error(transparent)]
    TomlDeError(#[from] toml::de::Error),

    #[error("Custom error: {0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, PonyError>;

impl From<String> for PonyError {
    fn from(err: String) -> Self {
        PonyError::Custom(err)
    }
}

impl<T: std::fmt::Debug> From<tokio::sync::mpsc::error::SendError<T>> for PonyError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        PonyError::Custom(format!("SendError: {:?}", err))
    }
}
