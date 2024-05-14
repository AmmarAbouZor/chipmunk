use std::path::Path;

use parsers::Parser;
use wasmtime::{
    component::{Component, Linker, ResourceAny},
    Config, Engine, Store,
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

use crate::PluginParseMessage;

use self::exports::host::parse::parsing::{Attachment, Error, ParseYield};

// This should be removed after prototyping
// File path should be read from config
const WASM_FILE_PATH: &str =
    "application/apps/indexer/wasm_plugin/client/target/wasm32-wasi/release/client.wasm";

wasmtime::component::bindgen!();

struct PluginState {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl PluginState {
    fn new(ctx: WasiCtx, table: ResourceTable) -> Self {
        Self { ctx, table }
    }
}

impl WasiView for PluginState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

// Suppress unused fields here while prototyping
#[allow(unused)]
pub struct WasmParser {
    engine: Engine,
    component: Component,
    linker: Linker<PluginState>,
    store: Store<PluginState>,
    parse_translate: Parse,
    parser_res: ResourceAny,
}

impl Drop for WasmParser {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        if let Err(err) = self.parser_res.resource_drop(&mut self.store) {
            log::error!("Error while dropping resources: {err}");
        }
    }
}

impl<'a> WasmParser {
    //TODO: Read plugin config from file after prototyping phase
    pub fn create(_config_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let wasm_path = std::env::current_dir()?.join("../..").join(WASM_FILE_PATH);
        dbg!(&wasm_path);
        anyhow::ensure!(
            wasm_path.exists(),
            "Wasm Plugin file doesn't exist. Path: {}",
            wasm_path.display()
        );

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(false);

        let engine = Engine::new(&config)?;

        let component = Component::from_file(&engine, wasm_path)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_sync(&mut linker)?;

        let ctx = WasiCtxBuilder::new().build();
        let table = ResourceTable::new();

        let mut store = Store::new(&engine, PluginState::new(ctx, table));

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
}

impl Parser<PluginParseMessage> for WasmParser {
    fn parse<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error> {
        let raw_res = self
            .parse_translate
            .interface0
            .parser()
            .call_parse(&mut self.store, self.parser_res, input, timestamp)
            //TODO: Change this after implementing error definitions
            .map_err(|err| {
                println!("TODO AAZ: Early Error: {err}");
                parsers::Error::Parse(err.to_string())
            })?;

        match raw_res {
            Ok(val) => {
                let remain = &input[val.cursor as usize..];
                let yld = val.value.map(|y| y.into_parsers_yield());

                // println!("TODO AAZ: remain: {remain:?}");
                // println!("TODO AAZ: yld: {yld:?}");

                Ok((remain, yld))
            }
            Err(err) => {
                println!("TODO AAZ: Error: {err}");
                Err(err.into_parsers_err())
            }
        }
    }
}

impl Attachment {
    fn into_parsers_attachment(self) -> parsers::Attachment {
        parsers::Attachment {
            data: self.data,
            name: self.name,
            size: self.size as usize,
            messages: self.messages.into_iter().map(|n| n as usize).collect(),
            created_date: self.created_date,
            modified_date: self.modified_date,
        }
    }
}

impl Error {
    fn into_parsers_err(self) -> parsers::Error {
        match self {
            Error::Parse(msg) => parsers::Error::Parse(msg),
            Error::Incomplete => parsers::Error::Incomplete,
            Error::Eof => parsers::Error::Eof,
        }
    }
}

impl ParseYield {
    fn into_parsers_yield(self) -> parsers::ParseYield<PluginParseMessage> {
        use parsers::ParseYield as HostYield;
        match self {
            ParseYield::Message(msg) => HostYield::Message(msg.into()),
            ParseYield::Attachment(att) => HostYield::Attachment(att.into_parsers_attachment()),
            ParseYield::MessageAndAttachment((msg, att)) => {
                HostYield::MessageAndAttachment((msg.into(), att.into_parsers_attachment()))
            }
        }
    }
}
