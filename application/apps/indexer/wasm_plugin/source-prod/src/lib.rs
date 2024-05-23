use exports::host::indexer::source_prod_client::Guest;
use file_source_prod::FileSourceProd;
use wit_bindgen::generate;

mod file_source_prod;

generate!({
    path: "../host/wit/indexer.wit",
    world: "producer",
});
struct Component;

impl Guest for Component {
    type SourceProd = FileSourceProd;
}

export!(Component);
