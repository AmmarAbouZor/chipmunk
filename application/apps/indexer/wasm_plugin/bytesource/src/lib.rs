use exports::host::indexer::source_client::Guest;
use file_source::FileSource;
use wit_bindgen::generate;

mod file_source;

generate!({
    path: "../host/wit/indexer.wit",
    world: "source",
});
struct Component;

impl Guest for Component {
    type ByteSource = FileSource;
}

export!(Component);
