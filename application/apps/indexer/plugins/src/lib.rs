//TODO AAZ: Suppress warnings while developing
#![allow(dead_code, unused_imports, unused)]

mod bytesoruce;
mod parser;
mod plugins_shared;
mod wasm_host;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    Parser,
    ByteSource,
}

pub trait WasmPlugin {
    fn get_type() -> PluginType;
}
