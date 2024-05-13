mod dlt_parser;

use crate::exports::host::parse::parsing::Guest;

use dlt_parser::DltParser;
use wit_bindgen::generate;

generate!({
    path: "../host/wit/parse.wit",
    world: "parse",
});

struct Component;

impl Guest for Component {
    type Parser = DltParser;
}

export!(Component);
