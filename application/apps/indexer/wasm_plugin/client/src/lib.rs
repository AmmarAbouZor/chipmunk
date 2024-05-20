mod dlt_parser;

use dlt_parser::DltParser;
use exports::host::parse::client::Guest;
use wit_bindgen::generate;

//TODO AAZ: Make sure we don't need ownership to be borrowing here.
generate!({
    path: "../host/wit/parse.wit",
    world: "parse",
    // ownership: Borrowing {
    //     duplicate_if_necessary: false
    // },
});

struct Component;

impl Guest for Component {
    type Parser = DltParser;
}

export!(Component);
