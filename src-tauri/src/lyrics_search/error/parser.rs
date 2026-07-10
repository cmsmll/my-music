/// 歌词解密失败。
#[derive(Debug, thiserror::Error)]
pub enum DecryptError {
    #[error("base64 decode failed: {detail} (len={len})")]
    Base64Decode { detail: String, len: usize },

    #[error("XOR decrypt failed: {detail}")]
    XorDecrypt { detail: String },

    #[error("deflate decompress failed: {detail}")]
    Deflate { detail: String },

    #[error("AES decrypt failed: {detail}")]
    AesDecrypt { detail: String },

    #[error("3DES decrypt failed: {detail}")]
    TripleDesDecrypt { detail: String },

    #[error("decrypted data is not valid UTF-8: {detail}")]
    Utf8Decode { detail: String },

    #[error("invalid key length: expected={expected}, actual={actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
}

/// 歌词文本解析失败。
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid lyrics structure: {detail}")]
    InvalidStructure { detail: String },

    #[error("timestamp parse error: field={field}, raw={raw}")]
    TimestampParse { field: String, raw: String },

    #[error("offset overflow: t1={t1}, t2={t2}")]
    OffsetOverflow { t1: u32, t2: u32 },

    #[error("syllable parse error: {detail}")]
    SyllableParse { detail: String },

    #[error("empty lyrics content")]
    EmptyContent,

    #[error("invalid LRC format: {detail}")]
    InvalidLrcFormat { detail: String },

    #[error("unknown lyrics sync type")]
    UnknownSyncType,
}

/// 解析器层错误。
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("{0}")]
    Parse(#[from] ParseError),

    #[error("{0}")]
    Decrypt(#[from] DecryptError),
}
