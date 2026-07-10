/// 通用/杂项错误。
#[derive(Debug, thiserror::Error)]
pub enum GeneralError {
    #[error("unsupported player: {name}")]
    UnsupportedPlayer { name: String },

    #[error("missing required field: {field}")]
    MissingField { field: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {detail}")]
    Internal { detail: String },

    #[error("platform error: {platform}")]
    Platform { platform: String },
}
