mod dlt_parser;

use dlt_parser::WasiDltParser;
use exports::host::parse::parsing::Guest;
use wit_bindgen::generate;

generate!({
    path: "../host/wit/parse.wit",
    world: "parse",
});

struct Component;

impl Guest for Component {
    type Parser = WasiDltParser;
}

export!(Component);
