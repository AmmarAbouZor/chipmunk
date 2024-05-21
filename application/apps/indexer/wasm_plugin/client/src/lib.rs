mod dlt_parser;

use dlt_parser::DltParser;
use exports::host::parse::parse_client::Guest;
use wit_bindgen::generate;

generate!({
    path: "../host/wit/parse.wit",
    world: "parse",
    //TODO AAZ: Activating borrowing gives better results with resource single values. But we can't
    // Use it use resource range method.
    // Activate this if we ended up using resource single value
    // ownership: Borrowing {
    //     duplicate_if_necessary: false
    // },
});

struct Component;

impl Guest for Component {
    type Parser = DltParser;
}

export!(Component);
