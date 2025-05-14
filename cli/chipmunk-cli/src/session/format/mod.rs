//! Provide traits and method for formatting the parsed messages.

use parsers::LogMessage;

pub mod binary;
pub mod duckdb;
pub mod sqlite;
pub mod text;

/// Method definitions for formatting and write parsed messages to the underline output.
pub trait MessageWriter {
    /// Format the message and then write it to the underline output.
    fn write_msg<M>(&mut self, msg: &M) -> anyhow::Result<()>
    where
        M: LogMessage;
    /// Flush all pending messages to the output source
    async fn flush(&mut self) -> anyhow::Result<()>;
}
