pub mod wrapper;

use anyhow::{anyhow, Context};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
};

use crate::{wasm_parser::Parse, GeneralState, PluginParseMessage};

use self::host::indexer::parsing::{Error, Host, HostResults, ParseReturn};
use wasmtime::{
    component::{Component, Linker, Resource, ResourceAny},
    Config, Engine, Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtxBuilder, WasiView};

type ParseResultIntern = Result<ParseReturn, Error>;

type ParseResult = Result<(usize, Option<parsers::ParseYield<PluginParseMessage>>), parsers::Error>;

// const PARSER_FILE_PATH: &str =
//     "application/apps/indexer/wasm_plugin/dlt-client/target/wasm32-wasi/release/dlt_client.wasm";
const PARSER_FILE_PATH: &str =
    "application/apps/indexer/wasm_plugin/client/target/wasm32-wasi/release/client.wasm";

const SOURCE_PROD_FILE_PATH: &str =
    "application/apps/indexer/wasm_plugin/source-prod/target/wasm32-wasi/release/source_prod.wasm";

const WASM_FILES_DIR: &str = "./files";

//TODO AAZ: Make sure we need ownership to be borrowing here
wasmtime::component::bindgen!({
    world: "producer",
    with: {
        "host:indexer/parsing/results": ResQueue,
    },
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
    async: {
        only_imports: [],
    },
});

impl HostResults for GeneralState {
    fn add(
        &mut self,
        queue: wasmtime::component::Resource<ResQueue>,
        item: Result<ParseReturn, Error>,
    ) {
        let queue = self
            .table()
            .get_mut(&queue)
            .expect("Queue is added to resource table");
        queue.queue.push_back(item);
    }

    fn add_range(
        &mut self,
        queue: wasmtime::component::Resource<ResQueue>,
        items: Vec<Result<ParseReturn, Error>>,
    ) {
        let queue = self
            .table()
            .get_mut(&queue)
            .expect("Queue is added to resource table");
        queue.queue = items.into();
    }

    fn drop(&mut self, rep: wasmtime::component::Resource<ResQueue>) -> wasmtime::Result<()> {
        self.table.delete(rep).expect("Queue is in resource table");
        Ok(())
    }
}

impl Host for GeneralState {}

#[derive(Default)]
pub struct ResQueue {
    pub queue: VecDeque<ParseResultIntern>,
}

// Suppress unused fields here while prototyping
#[allow(unused)]
pub struct WasmProducer {
    engine: Engine,
    source_prod_component: Component,
    parser_component: Component,
    linker: Linker<GeneralState>,
    store: Store<GeneralState>,
    parse_translate: Parse,
    parser_res: ResourceAny,
    source_prod_translate: Producer,
    source_prod_res: ResourceAny,
    queue_res: Resource<ResQueue>,
}

impl Drop for WasmProducer {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        for res in [self.parser_res, self.source_prod_res] {
            if let Err(err) = futures::executor::block_on(res.resource_drop_async(&mut self.store))
            {
                log::error!("Error while dropping resources: {err}");
            }
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
        let wasm_parser_path = get_valid_path(PARSER_FILE_PATH)?;

        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;

        let source_prod_component = Component::from_file(&engine, wasm_soruce_prod_path)?;
        let parser_component = Component::from_file(&engine, wasm_parser_path)?;

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

        let mut store = Store::new(&engine, GeneralState::new(ctx, table));

        let queue_res = store.data_mut().table().push(ResQueue::default()).unwrap();

        let (parse_translate, _instance) =
            Parse::instantiate_async(&mut store, &parser_component, &linker).await?;

        let parser_res = parse_translate
            .host_indexer_parse_client()
            .parser()
            .call_constructor(&mut store)
            .await?;

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
            parser_component,
            linker,
            store,
            parse_translate,
            parser_res,
            source_prod_translate,
            source_prod_res,
            queue_res,
        })
    }

    pub async fn read_next(&mut self) -> Option<ParseResult> {
        todo!()
    }
}
