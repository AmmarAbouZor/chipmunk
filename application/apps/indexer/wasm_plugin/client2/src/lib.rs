mod dlt_parser;

use dlt_parser::DltParser;
use exports::host::indexer::parse2_client::Guest;
use wit_bindgen::generate;

generate!({
    path: "../host/wit/indexer.wit",
    world: "parse2",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
});

struct Component;

impl Guest for Component {
    type Parser = DltParser;
}

export!(Component);
