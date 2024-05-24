pub mod wrapper;

use anyhow::{anyhow, Context};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use crate::PluginParseMessage;

use self::host::indexer::parsing::{self, Attachment, Error, ParseReturn, ParseYield};
use wasmtime::{
    component::{Component, Linker, ResourceAny},
    Config, Engine, Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

type ParseResult = Result<ParseReturn, Error>;

type ParseResultExtern =
    Result<(usize, Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error>;

const SOURCE_PROD_FILE_PATH: &str = "application/apps/indexer/wasm_plugin/plugged.wasm";

const WASM_FILES_DIR: &str = "./files";

// Taken from soruces/src/lib.rs
pub(crate) const DEFAULT_READER_CAPACITY: u64 = 10 * 1024 * 1024;

//TODO AAZ: Make sure we need ownership to be borrowing here
wasmtime::component::bindgen!({
    world: "producer",
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
pub struct WasmProducer {
    engine: Engine,
    source_prod_component: Component,
    linker: Linker<MyParserState>,
    store: Store<MyParserState>,
    source_prod_translate: Producer,
    source_prod_res: ResourceAny,
    read_count: u64,
}

impl Drop for WasmProducer {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        if let Err(err) =
            futures::executor::block_on(self.source_prod_res.resource_drop_async(&mut self.store))
        {
            log::error!("Error while dropping resources: {err}");
        }
    }
}

fn get_valid_path(path_str: &str) -> anyhow::Result<PathBuf> {
    // assume we are calling the function from indexer-cli
    let mut file_path = std::env::current_dir()?.join("../../../..").join(path_str);
    // if not indexer-cli then assume we are calling it from rake in root directory
    if !file_path.exists() {
        file_path = std::env::current_dir()?.join("../..").join(path_str);
    }
    anyhow::ensure!(
        file_path.exists(),
        "Wasm Plugin file doesn't exist. Path: {}",
        file_path.display()
    );

    Ok(file_path)
}

impl WasmProducer {
    pub async fn create(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let wasm_soruce_prod_path = get_valid_path(SOURCE_PROD_FILE_PATH)?;

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let source_prod_component = Component::from_file(&engine, wasm_soruce_prod_path)?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        self::host::indexer::parsing::add_to_linker(&mut linker, |state| state)?;

        let file_path = file_path.as_ref();
        let path_dir = file_path.parent().ok_or_else(|| {
            anyhow!(
                "Can't resolve file parent. File path: {}",
                file_path.display()
            )
        })?;

        let ctx = WasiCtxBuilder::new()
            .inherit_stdin()
            .inherit_stdout()
            .inherit_stderr()
            .preopened_dir(path_dir, WASM_FILES_DIR, DirPerms::READ, FilePerms::READ)?
            .build();

        let table = ResourceTable::new();

        let mut store = Store::new(&engine, MyParserState::new(ctx, table));

        let (source_prod_translate, _instance) =
            Producer::instantiate_async(&mut store, &source_prod_component, &linker).await?;

        let source_prod_res = source_prod_translate
            .host_indexer_source_prod_client()
            .source_prod()
            .call_constructor(&mut store)
            .await?;

        let file_name = file_path
            .file_name()
            .ok_or_else(|| anyhow!("Couldn't get file name. Path: {}", file_path.display()))?;

        let file_path_guest = PathBuf::from(WASM_FILES_DIR).join(file_name);

        source_prod_translate
            .host_indexer_source_prod_client()
            .source_prod()
            .call_init(
                &mut store,
                source_prod_res,
                "".into(),
                &file_path_guest.to_string_lossy(),
            )
            .await
            .context("Error while initializing source source reader")??;

        Ok(Self {
            engine,
            source_prod_component,
            linker,
            store,
            source_prod_translate,
            source_prod_res,
            read_count: 0,
        })
    }

    pub async fn read_next(&mut self) -> Option<ParseResultExtern> {
        let queue = &mut self.store.data_mut().queue;
        let raw_res = match queue.pop_front() {
            // In case of errors we send the whole slice again. This could be optimized to reduce
            // the calls to wasm
            None | Some(Err(Error::Parse(_))) | Some(Err(Error::Incomplete)) => {
                self.source_prod_translate
                    .interface0
                    .source_prod()
                    .call_read_then_parse(
                        &mut self.store,
                        self.source_prod_res,
                        DEFAULT_READER_CAPACITY,
                        self.read_count,
                        None,
                    )
                    .await
                    //TODO: Change this after implementing error definitions
                    .map_err(|err| {
                        println!("TODO AAZ: Early Error: {err}");
                        parsers::Error::Parse(err.to_string())
                    })
                    .unwrap()
                    .unwrap();
                self.read_count = 0;
                let queue = &mut self.store.data_mut().queue;
                queue.pop_front().unwrap()
            }
            Some(res) => res,
        };

        match raw_res {
            Ok(val) => {
                self.read_count += val.cursor;
                let yld = val.value.map(|y| y.into_parsers_yield());

                let not_used_offset = 0;

                Some(Ok((not_used_offset, yld)))
            }
            Err(_) => {
                // println!("TODO AAZ: Error: {err:?}");
                None
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
