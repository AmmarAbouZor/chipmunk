use std::{fmt::Display, path::Path};

use parsers::{LogMessage, Parser};
use serde::Serialize;

pub mod dlt;

pub trait PluginParser: Parser<PluginParseMessage> {
    fn create(config_path: impl AsRef<Path>) -> Self;
}

#[derive(Debug, Serialize)]
pub struct PluginParseMessage {
    content: String,
}

impl PluginParseMessage {
    pub fn new(content: String) -> Self {
        Self { content }
    }
}

impl From<String> for PluginParseMessage {
    fn from(content: String) -> Self {
        Self::new(content)
    }
}

impl From<PluginParseMessage> for String {
    fn from(value: PluginParseMessage) -> Self {
        value.content
    }
}

impl Display for PluginParseMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

impl LogMessage for PluginParseMessage {
    fn to_writer<W: std::io::prelude::Write>(
        &self,
        writer: &mut W,
    ) -> Result<usize, std::io::Error> {
        let bytes = self.content.as_bytes();
        let len = bytes.len();
        writer.write_all(bytes)?;
        Ok(len)
    }
}
