use crate::exports::host::parse::parsing;
use crate::exports::host::parse::parsing::Guest;
use crate::exports::host::parse::parsing::ParseReturn;

use exports::host::parse::parsing::GuestParser;
use wit_bindgen::generate;

generate!({
    path: "../host/wit/parse.wit",
    world: "parse",
});

struct Component;

impl Guest for Component {
    type Parser = DltParser;
}

struct DltParser;

impl GuestParser for DltParser {
    fn new() -> Self {
        Self
    }

    fn parse(&self, data: Vec<u8>, timestamp: Option<u64>) -> Result<ParseReturn, parsing::Error> {
        todo!()
    }
}

export!(Component);
