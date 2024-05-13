use parsers::{ParseYield, Parser};

use super::{PluginParseMessage, PluginParser};

#[derive(Debug)]
pub struct DltWasmParser;

//TODO AAZ: Make sure we release the resource in WASM when the parser is dropped

impl PluginParser for DltWasmParser {
    //TODO: Read plugin config from file
    fn create(_config_path: impl AsRef<std::path::Path>) -> Self {
        todo!()
    }
}

impl Parser<PluginParseMessage> for DltWasmParser {
    fn parse<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<ParseYield<PluginParseMessage>>), parsers::Error> {
        todo!()
    }
}
