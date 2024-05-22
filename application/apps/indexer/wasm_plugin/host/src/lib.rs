mod wasm_bytesource;
mod wasm_parser;

use std::fmt::Display;

use parsers::LogMessage;
use serde::Serialize;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiView};

pub use wasm_bytesource::WasmByteSource;
pub use wasm_parser::WasmParser;

/// Represents which method should be used with the parsing. This is used in the experimental phase  
pub enum ParseMethod {
    /// Return The items as a vector directly
    ReturnVec,
    /// Pass a resource from the host to be filled in the client once at a time
    ResSingle,
    /// Pass a resource from the host to be filled in the client once all items in the given slice
    /// has been parsed
    ResRange,
}

impl Display for ParseMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseMethod::ReturnVec => write!(f, "Return Vector"),
            ParseMethod::ResSingle => write!(f, "Resource Single"),
            ParseMethod::ResRange => write!(f, "Resource Range"),
        }
    }
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

pub(crate) struct GeneralState {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
}

impl GeneralState {
    pub fn new(ctx: WasiCtx, table: ResourceTable) -> Self {
        Self { ctx, table }
    }
}

impl WasiView for GeneralState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}
