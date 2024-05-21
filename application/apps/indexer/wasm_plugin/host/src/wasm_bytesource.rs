use std::path::Path;

use anyhow::anyhow;
use wasmtime::{
    component::{Component, Linker, ResourceAny},
    Config, Engine, Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtxBuilder};

use crate::GeneralState;

const WASM_FILE_PATH: &str =
    "application/apps/indexer/wasm_plugin/bytesource/target/wasm32-wasi/release/bytesource.wasm";
const WASM_FILES_DIR: &str = "./files";

//TODO AAZ: Make sure we need ownership to be borrowing here
wasmtime::component::bindgen!({
    world: "source",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
    async: {
        only_imports: [],
    },
});

// Suppress unused fields here while prototyping
#[allow(unused)]
pub struct WasmByteSource {
    engine: Engine,
    component: Component,
    linker: Linker<GeneralState>,
    store: Store<GeneralState>,
    source_translate: Source,
    source_res: ResourceAny,
}

impl Drop for WasmByteSource {
    fn drop(&mut self) {
        // It's required to call drop on the resource Parser instance manually
        if let Err(err) =
            futures::executor::block_on(self.source_res.resource_drop_async(&mut self.store))
        {
            log::error!("Error while dropping resources: {err}");
        }
    }
}

impl WasmByteSource {
    pub async fn create(
        file_path: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
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

        // self::host::indexer::sourcing::add_to_linker(&mut linker, |state| state);

        let path_dir = file_path.as_ref().parent().ok_or_else(|| {
            anyhow!(
                "Can't resolve file parent. File path: {}",
                file_path.as_ref().display()
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

        // let queue_res = store.data_mut().table().push(ResQueue::default()).unwrap();

        let (source_translate, _instance) =
            Source::instantiate_async(&mut store, &component, &linker).await?;

        let source_res = source_translate
            .interface0
            .byte_source()
            .call_constructor(&mut store, &config_path.as_ref().to_string_lossy())
            .await?;

        Ok(Self {
            engine,
            component,
            linker,
            store,
            source_translate,
            source_res,
        })
    }
}
