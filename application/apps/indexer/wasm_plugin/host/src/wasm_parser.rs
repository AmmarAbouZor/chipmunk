use std::{collections::VecDeque, path::Path, usize};

use parsers::Parser;
use wasmtime::{
    component::{Component, Linker, ResourceAny},
    Config, Engine, Store,
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

use crate::{ParseMethod, PluginParseMessage};

use self::host::indexer::parsing::{self, Attachment, Error, ParseReturn, ParseYield};

type ParseResult = Result<ParseReturn, Error>;

// This should be removed after prototyping
// File path should be read from config
// const WASM_FILE_PATH: &str =
//     "application/apps/indexer/wasm_plugin/dlt-client/target/wasm32-wasi/release/dlt_client.wasm";
const WASM_FILE_PATH: &str =
    "application/apps/indexer/wasm_plugin/client/target/wasm32-wasi/release/client.wasm";

//TODO AAZ: Make sure we need ownership to be borrowing here
wasmtime::component::bindgen!({
    world: "parse",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
    async: {
        only_imports: [],
    },
});

struct MyParserState {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
    pub queue: VecDeque<ParseResult>,
}

impl MyParserState {
    pub fn new(ctx: WasiCtx, table: ResourceTable) -> Self {
        Self {
            ctx,
            table,
            queue: Default::default(),
        }
    }
}

impl WasiView for MyParserState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl parsing::Host for MyParserState {
    fn add(&mut self, item: Result<ParseReturn, Error>) -> () {
        self.queue.push_back(item);
    }

    fn add_range(
        &mut self,
        items: wasmtime::component::__internal::Vec<Result<ParseReturn, Error>>,
    ) -> () {
        assert!(self.queue.is_empty());
        self.queue = items.into();
    }
}

// Suppress unused fields here while prototyping
#[allow(unused)]
pub struct WasmParser {
    engine: Engine,
    component: Component,
    linker: Linker<MyParserState>,
    store: Store<MyParserState>,
    parse_translate: Parse,
    parser_res: ResourceAny,
    cache: VecDeque<ParseResult>,
    method: ParseMethod,
}

impl Drop for WasmParser {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        if let Err(err) =
            futures::executor::block_on(self.parser_res.resource_drop_async(&mut self.store))
        {
            log::error!("Error while dropping resources: {err}");
        }
    }
}

// Suppress unused functions here while prototyping
#[allow(unused)]
impl WasmParser {
    //TODO: Read plugin config from file after prototyping phase
    pub async fn create(
        _config_path: impl AsRef<Path>,
        method: ParseMethod,
    ) -> anyhow::Result<Self> {
        // assume we are calling the function from indexer-cli
        let mut wasm_path = std::env::current_dir()?
            .join("../../../..")
            .join(WASM_FILE_PATH);
        // if not indexer-cli then assume we are calling it from rake in root directory
        if !wasm_path.exists() {
            wasm_path = std::env::current_dir()?.join("../..").join(WASM_FILE_PATH);
        }
        dbg!(&wasm_path);
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

        self::host::indexer::parsing::add_to_linker(&mut linker, |state| state);

        let ctx = WasiCtxBuilder::new().build();
        let table = ResourceTable::new();

        let mut store = Store::new(&engine, MyParserState::new(ctx, table));

        let (parse_translate, _instance) =
            Parse::instantiate_async(&mut store, &component, &linker).await?;

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
            cache: VecDeque::new(),
            method,
        })
    }

    #[inline]
    fn parse_with_list<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error> {
        let raw_res = match self.cache.pop_front() {
            // In case of errors we send the whole slice again. This could be optimized to reduce
            // the calls to wasm
            None | Some(Err(Error::Parse(_))) | Some(Err(Error::Incomplete)) => {
                let results = futures::executor::block_on(
                    self.parse_translate.interface0.parser().call_parse(
                        &mut self.store,
                        self.parser_res,
                        input,
                        timestamp,
                    ),
                )
                //TODO: Change this after implementing error definitions
                .map_err(|err| {
                    println!("TODO AAZ: Early Error: {err}");
                    parsers::Error::Parse(err.to_string())
                })?;
                self.cache = results.into();
                self.cache
                    .pop_front()
                    .expect("Wasm always returns semothing")
            }
            Some(res) => res,
        };

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

    #[inline]
    fn parse_with_res<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error> {
        let queue = &mut self.store.data_mut().queue;
        let raw_res = match queue.pop_front() {
            // In case of errors we send the whole slice again. This could be optimized to reduce
            // the calls to wasm
            None | Some(Err(Error::Parse(_))) | Some(Err(Error::Incomplete)) => {
                futures::executor::block_on(
                    self.parse_translate.interface0.parser().call_parse_res(
                        &mut self.store,
                        self.parser_res,
                        input,
                        timestamp,
                    ),
                )
                //TODO: Change this after implementing error definitions
                .map_err(|err| {
                    println!("TODO AAZ: Early Error: {err}");
                    parsers::Error::Parse(err.to_string())
                })?;
                return self.parse_with_res(input, timestamp);
            }
            Some(res) => res,
        };

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

    #[inline]
    fn parse_with_res_rng<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error> {
        let queue = &mut self.store.data_mut().queue;
        let raw_res = match queue.pop_front() {
            // In case of errors we send the whole slice again. This could be optimized to reduce
            // the calls to wasm
            None | Some(Err(Error::Parse(_))) | Some(Err(Error::Incomplete)) => {
                futures::executor::block_on(
                    self.parse_translate.interface0.parser().call_parse_res_rng(
                        &mut self.store,
                        self.parser_res,
                        input,
                        timestamp,
                    ),
                )
                //TODO: Change this after implementing error definitions
                .map_err(|err| {
                    println!("TODO AAZ: Early Error: {err}");
                    parsers::Error::Parse(err.to_string())
                })?;
                return self.parse_with_res_rng(input, timestamp);
            }
            Some(res) => res,
        };

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

impl Parser<PluginParseMessage> for WasmParser {
    fn parse<'a>(
        &mut self,
        input: &'a [u8],
        timestamp: Option<u64>,
    ) -> Result<(&'a [u8], Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error> {
        //TODO AAZ: Currently I'm using parse_with_res because it has the best perfomance on my
        //machine, but I need to test the other approaches on another machine
        self.parse_with_res(input, timestamp)
        // match self.method {
        //     ParseMethod::ReturnVec => self.parse_with_list(input, timestamp),
        //     ParseMethod::ResSingle => self.parse_with_res(input, timestamp),
        //     ParseMethod::ResRange => self.parse_with_res_rng(input, timestamp),
        // }
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
