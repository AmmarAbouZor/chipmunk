//! Provides function to create and configure multiple parsers.

pub mod dlt;

/// Represents Information and configurations for a specific parser.
#[derive(Debug, Clone)]
pub struct ParserInfo {
    /// The columns of the parsed records.
    pub columns: Vec<ColumnInfo>,
}

impl ParserInfo {
    pub fn new(columns: Vec<ColumnInfo>) -> Self {
        Self { columns }
    }
}

/// The columns infos for parsers records.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub caption: String,
}

impl ColumnInfo {
    pub fn new(caption: impl Into<String>) -> Self {
        Self {
            caption: caption.into(),
        }
    }
}
