use std::path::{Path, PathBuf};

use wasmtime::{
    component::{Component, Linker, ResourceAny},
    Config, Engine, Store,
};

pub use self::exports::host::parse::parsing::{Attachment, Error, ParseReturn, ParseYield};

// This should be removed after prototyping
// File path should be read from config
const WASM_FILE_PATH: &str = "../client/target/wasm32-unknown-unknown/release/client.wasm";

wasmtime::component::bindgen!();

struct PluginState;

// Suppress unused fields here while prototyping
#[allow(unused)]
pub struct DltParser {
    engine: Engine,
    component: Component,
    linker: Linker<PluginState>,
    store: Store<PluginState>,
    parse_translate: Parse,
    parser_res: ResourceAny,
}

impl Drop for DltParser {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        if let Err(err) = self.parser_res.resource_drop(&mut self.store) {
            log::error!("Error while dropping resources: {err}");
        }
    }
}

impl<'a> DltParser {
    //TODO: Read plugin config from file after prototyping phase
    pub fn create(_config_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let wasm_path = PathBuf::from(WASM_FILE_PATH);
        anyhow::ensure!(
            wasm_path.exists(),
            "Wasm Plugin file doesn't exist. Path: {}",
            wasm_path.display()
        );

        let mut config = Config::new();
        config.wasm_component_model(true);

        let engine = Engine::new(&config)?;

        let component = Component::from_file(&engine, wasm_path)?;

        let linker = Linker::new(&engine);

        let mut store = Store::new(&engine, PluginState);

        let (parse_translate, _instance) = Parse::instantiate(&mut store, &component, &linker)?;

        let parser_res = parse_translate
            .interface0
            .parser()
            .call_constructor(&mut store)?;

        Ok(Self {
            engine,
            component,
            linker,
            store,
            parse_translate,
            parser_res,
        })
    }

    pub fn parse(
        &mut self,
        data: &[u8],
        timestamp: Option<u64>,
    ) -> anyhow::Result<Result<ParseReturn, Error>> {
        // let test = parsers

        self.parse_translate.interface0.parser().call_parse(
            &mut self.store,
            self.parser_res,
            data,
            timestamp,
        )
    }
}
