use std::fmt;

/// HTTP 请求错误，不直接暴露 reqwest::Error。
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("bad request (400): {url}")]
    BadRequest { url: String },

    #[error("unauthorized (401): {url}")]
    Unauthorized { url: String },

    #[error("forbidden (403): {url}")]
    Forbidden { url: String },

    #[error("not found (404): {url}")]
    NotFound { url: String },

    #[error("too many requests (429): {url}")]
    TooManyRequests { url: String },

    #[error("server error (500): {url}")]
    ServerError { url: String },

    #[error("bad gateway (502): {url}")]
    BadGateway { url: String },

    #[error("service unavailable (503): {url}")]
    ServiceUnavailable { url: String },

    #[error("unexpected redirect ({status}): {url}")]
    Redirect { status: u16, url: String },

    #[error("HTTP {status}: {url}")]
    OtherStatus { status: u16, url: String },

    #[error("connection failed: {detail} (url={url})")]
    ConnectionFailed { detail: String, url: String },

    #[error("request timeout: {url}")]
    Timeout { url: String },

    #[error("TLS error: {detail} (url={url})")]
    TlsError { detail: String, url: String },
}

/// JSON 反序列化错误，包装 serde_json::Error 和 API 上下文。
#[derive(Debug)]
pub struct JsonError {
    pub api: String,
    pub source: serde_json::Error,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} API response parse failed: {}", self.api, self.source)
    }
}

impl std::error::Error for JsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// 提供器层错误。
#[derive(Debug, thiserror::Error)]
pub enum FetcherError {
    #[error("{0}")]
    Http(#[from] HttpError),

    #[error("{0}")]
    Json(#[from] JsonError),
}
