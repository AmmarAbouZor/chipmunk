use std::{slice, usize};

use parsers::Parser;
use wasmtime::{
    component::{Component, Linker, ResourceAny},
    Config, Engine, Store,
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

use crate::PluginParseMessage;

use self::host::indexer::parsing::{Attachment, Error, ParseYield};

const WASM_FILE_PATH: &str =
    "application/apps/indexer/wasm_plugin/client2/target/wasm32-wasi/release/client2.wasm";

//TODO AAZ: Make sure we need ownership to be borrowing here
wasmtime::component::bindgen!({
    world: "parse2",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
    async: {
        only_imports: [],
    },
});

pub(crate) struct ParserState {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
    pub slice_ptr: usize,
    pub slice_len: usize,
}

impl ParserState {
    pub fn new(ctx: WasiCtx, table: ResourceTable) -> Self {
        Self {
            ctx,
            table,
            slice_ptr: Default::default(),
            slice_len: Default::default(),
        }
    }
}

impl WasiView for ParserState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl source_host::Host for ParserState {
    fn get_data(&mut self) -> wasmtime::component::__internal::Vec<u8> {
        let slice = unsafe { slice::from_raw_parts(self.slice_ptr as *const _, self.slice_len) };

        slice.into()
    }
}

// Suppress unused fields here while prototyping
#[allow(unused)]
pub struct WasmParser2 {
    engine: Engine,
    component: Component,
    linker: Linker<ParserState>,
    store: Store<ParserState>,
    parse_translate: Parse2,
    parser_res: ResourceAny,
}

impl Drop for WasmParser2 {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        if let Err(err) =
            futures::executor::block_on(self.parser_res.resource_drop_async(&mut self.store))
        {
            log::error!("Error while dropping resources: {err}");
        }
    }
}

impl WasmParser2 {
    pub async fn create() -> anyhow::Result<Self> {
        // assume we are calling the function from indexer-cli
        let mut wasm_path = std::env::current_dir()?
            .join("../../../..")
            .join(WASM_FILE_PATH);
        // if not indexer-cli then assume we are calling it from rake in root directory
        if !wasm_path.exists() {
            wasm_path = std::env::current_dir()?.join("../..").join(WASM_FILE_PATH);
        }
        anyhow::ensure!(
            wasm_path.exists(),
            "Wasm Plugin file doesn't exist. Path: {}",
            wasm_path.display()
        );

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let component = Component::from_file(&engine, wasm_path)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        source_host::add_to_linker(&mut linker, |state| state).unwrap();

        let ctx = WasiCtxBuilder::new()
            .inherit_stdin()
            .inherit_stdout()
            .inherit_stderr()
            .build();
        let table = ResourceTable::new();

        let mut store = Store::new(&engine, ParserState::new(ctx, table));

        let (parse_translate, _instance) =
            Parse2::instantiate_async(&mut store, &component, &linker).await?;

        let parser_res = parse_translate
            .interface0
            .parser()
            .call_constructor(&mut store)
            .await?;

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

impl Parser<PluginParseMessage> for WasmParser2 {
    fn parse<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error> {
        let state = self.store.data_mut();
        state.slice_ptr = input.as_ptr() as usize;
        state.slice_len = input.len();

        let raw_res =
            futures::executor::block_on(self.parse_translate.interface0.parser().call_parse_next(
                &mut self.store,
                self.parser_res,
                timestamp,
            ))
            .unwrap();

        match raw_res {
            Ok(val) => {
                let remain = &input[val.cursor as usize..];
                let yld = val.value.map(|y| y.into_parsers_yield());

                Ok((remain, yld))
            }
            Err(err) => {
                let err = err.into_parsers_err();
                // println!("TODO AAZ: Error: {err}");
                Err(err)
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
