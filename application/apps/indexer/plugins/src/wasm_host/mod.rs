// TODO: Temporally place holder
#![allow(dead_code, unused_imports, unused)]

use thiserror::Error;
use tokio::sync::OnceCell;
use wasmtime::{Config, Engine};

#[derive(Debug, Error)]
pub enum WasmInitError {
    #[error("Error while initializing WASM Engine")]
    EngineError(#[from] anyhow::Error),
}

pub struct WasmHost {
    engine: Engine,
}

impl WasmHost {
    fn init() -> Result<Self, WasmInitError> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        //TODO AAZ: Check the impact of these when we have a proper benchmarks
        // config.relaxed_simd_deterministic(true);
        // config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)?;

        let host = Self { engine };

        Ok(host)
    }
}

pub async fn get_wasm_host() -> &'static Result<WasmHost, WasmInitError> {
    static WASM_HOST: OnceCell<Result<WasmHost, WasmInitError>> = OnceCell::const_new();

    WASM_HOST.get_or_init(|| async { WasmHost::init() }).await
}
