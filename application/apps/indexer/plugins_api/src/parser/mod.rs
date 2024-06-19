// TODO: Temporally place holder
#![allow(dead_code, unused_imports, unused)]

use wit_bindgen::generate;

generate!({
    path: "wit/v_0.1.0",
    world: "parser",
});

struct PluginParser;

impl Guest for PluginParser {
    /// Initialize the parser with the given configurations
    fn init(configs: _rt::String) -> Result<(), InitError> {
        todo!()
    }

    /// Parse the given bytes returning a list of plugins results
    fn parse(
        data: _rt::Vec<u8>,
        timestamp: Option<u64>,
    ) -> _rt::Vec<Result<ParseReturn, ParseError>> {
        todo!()
    }

    /// Parse the given bytes returning the results to the host one by one using the function `add` provided by the host.
    fn parse_with_add(data: _rt::Vec<u8>, timestamp: Option<u64>) {
        todo!()
    }
}
