use std::path::{Path, PathBuf};

use thiserror::Error;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use crate::{parser::PluginGuestInitError, wasm_host::WasmHostInitError};

/// Path of plugin configurations directory that will presented to the plugins.
const PLUGINS_CONFIG_DIR_PATH: &str = "./config";

#[derive(Debug, Error)]
pub enum PluginHostInitError {
    #[error("Error while initializing WASM Engine. {0}")]
    EngineError(#[from] WasmHostInitError),
    #[error("Validating the plugin while loading failed. {0}")]
    PluginInvalid(String),
    #[error("Error reported from the plugin")]
    GuestError(PluginGuestInitError),
    #[error("IO Error while initializing WASM Plugin. {0}")]
    IO(String),
    #[error(transparent)]
    WasmRunTimeError(#[from] anyhow::Error),
}

/// Creates [`WasiCtxBuilder`] with shared configurations, giving the plugin access to their
/// configurations file directory.
pub fn get_wasi_ctx_builder(
    config_path: impl AsRef<Path>,
) -> Result<WasiCtxBuilder, PluginHostInitError> {
    let config_path = config_path.as_ref();
    let config_dir = config_path.parent().ok_or(PluginHostInitError::IO(
        "Resolve config file parent failed".into(),
    ))?;

    let mut ctx = WasiCtxBuilder::new();
    ctx.inherit_stdout().inherit_stderr().preopened_dir(
        config_dir,
        PLUGINS_CONFIG_DIR_PATH,
        DirPerms::READ,
        FilePerms::READ,
    )?;

    Ok(ctx)
}

/// Get plugin configuration path as it should be presented to the plugin
pub fn get_plugin_config_path(
    real_config_path: impl AsRef<Path>,
) -> Result<PathBuf, PluginHostInitError> {
    let file_name = real_config_path
        .as_ref()
        .file_name()
        .ok_or_else(|| PluginHostInitError::IO("Resolve config file name failed".into()))?;

    let plugin_config_path = PathBuf::from(PLUGINS_CONFIG_DIR_PATH).join(file_name);

    Ok(plugin_config_path)
}
