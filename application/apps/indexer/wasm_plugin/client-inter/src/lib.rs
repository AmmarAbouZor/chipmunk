mod dlt_parser;

use dlt_parser::DltParser;
use exports::host::indexer::parse_client_inter::Guest;
use host::indexer::parsing::{Error, ParseReturn};
use lazy_static::lazy_static;
use wit_bindgen::generate;

generate!({
    path: "../host/wit/indexer.wit",
    world: "parse-inter",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
});

lazy_static! {
    static ref PARSER: DltParser = DltParser::new();
}

struct Component;

impl Guest for Component {
    fn init(configs: _rt::String) -> Result<(), Error> {
        Ok(PARSER.init(configs))
    }

    fn parse(data: _rt::Vec<u8>, timestamp: Option<u64>) -> _rt::Vec<Result<ParseReturn, Error>> {
        PARSER.parse(data, timestamp)
    }

    fn parse_res(data: _rt::Vec<u8>, timestamp: Option<u64>) {
        PARSER.parse_res(data, timestamp)
    }
}

export!(Component);
