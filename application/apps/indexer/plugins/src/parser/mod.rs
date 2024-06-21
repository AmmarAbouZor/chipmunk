mod bindings;
mod parser_plugin_state;
mod plugin_parse_message;

use std::path::Path;

use wasmtime::{
    component::{Component, Linker},
    Store,
};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtxBuilder};

use crate::{
    plugins_shared::{get_plugin_config_path, get_wasi_ctx_builder, PluginHostInitError},
    wasm_host::get_wasm_host,
    PluginType, WasmPlugin,
};

use self::{bindings::Parser, parser_plugin_state::ParserPluginState};

pub use plugin_parse_message::PluginParseMessage;

pub use bindings::{InitError as PluginGuestInitError, ParserConfig};

pub struct PluginParser {
    store: Store<ParserPluginState>,
    plugin_bindings: Parser,
}

impl WasmPlugin for PluginParser {
    fn get_type() -> PluginType {
        PluginType::Parser
    }
}

impl PluginParser {
    pub async fn create(
        plugin_path: impl AsRef<Path>,
        general_config: ParserConfig,
        config_path: impl AsRef<Path>,
    ) -> Result<Self, PluginHostInitError> {
        let engine = match get_wasm_host().await {
            Ok(host) => &host.engine,
            Err(err) => return Err(err.to_owned().into()),
        };

        let component = Component::from_file(engine, plugin_path)
            .map_err(|err| PluginHostInitError::PluginInvalid(err.to_string()))?;

        let mut linker: Linker<ParserPluginState> = Linker::new(engine);
        wasmtime_wasi::add_to_linker_async(&mut linker)?;

        Parser::add_to_linker(&mut linker, |state| state);

        let mut ctx = get_wasi_ctx_builder(&config_path)?;
        let resource_table = ResourceTable::new();

        let mut store = Store::new(&engine, ParserPluginState::new(ctx.build(), resource_table));

        let (plugin_bindings, _instance) =
            Parser::instantiate_async(&mut store, &component, &linker).await?;

        let plugin_config_path = get_plugin_config_path(config_path)?;
        let plugin_config_path = plugin_config_path.to_str().ok_or_else(|| {
            PluginHostInitError::IO(format!(
                "Plugin Config Path isn't valid utf-8 string: {}",
                plugin_config_path.display()
            ))
        })?;

        plugin_bindings
            .call_init(&mut store, &general_config, plugin_config_path)
            .await?
            .map_err(|guest_err| PluginHostInitError::GuestError(guest_err))?;

        Ok(Self {
            store,
            plugin_bindings,
        })
    }
}
