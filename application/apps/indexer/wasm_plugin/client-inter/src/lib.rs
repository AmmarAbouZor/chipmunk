mod dlt_parser;

use std::sync::OnceLock;

use dlt_parser::DltParser;
use exports::host::indexer::parse_client_inter::Guest;
use host::indexer::parsing::{Error, ParseReturn};
use wit_bindgen::generate;

generate!({
    path: "../host/wit/indexer.wit",
    world: "parse-inter",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
});

static PARSER: OnceLock<DltParser> = OnceLock::new();

struct Component;

impl Guest for Component {
    fn init(configs: _rt::String) -> Result<(), Error> {
        let parser = DltParser::new();
        parser.init(configs);
        PARSER.set(parser).unwrap();
        Ok(())
    }

    fn parse(data: _rt::Vec<u8>, timestamp: Option<u64>) -> _rt::Vec<Result<ParseReturn, Error>> {
        PARSER.get().unwrap().parse(data, timestamp)
    }

    fn parse_res(data: _rt::Vec<u8>, timestamp: Option<u64>) {
        PARSER.get().unwrap().parse_res(data, timestamp)
    }
}

export!(Component);
