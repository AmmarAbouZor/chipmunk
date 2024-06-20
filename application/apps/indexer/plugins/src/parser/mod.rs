mod plugin_parse_message;
mod plugin_parser_state;

pub use plugin_parse_message::PluginParseMessage;

mod binding {
    wasmtime::component::bindgen!({
        path: "../plugins_api/wit/v_0.1.0/",
        world: "parser",
        ownership: Borrowing {
            duplicate_if_necessary: false
        },
        async: {
            only_imports: [],
        },
    });
}
