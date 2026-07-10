/// 搜索器层错误。
#[derive(Debug, thiserror::Error)]
pub enum SearcherError {
    #[error("{label}: no search results (query={query})")]
    NoResults { label: String, query: String },

    #[error("{label}: score too low (best={score}/{threshold}, query={query})")]
    LowScore {
        label: String,
        score: i8,
        threshold: i8,
        query: String,
    },

    #[error("{label}: no matching track found (title={title})")]
    NoMatch { label: String, title: String },

    #[error("{label}: missing required field ({field})")]
    MissingField { label: String, field: String },
}
