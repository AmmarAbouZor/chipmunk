use crate::{ParserConfig as HostParseConfig, PluginGuestInitError, PluginHostInitError};

use self::chipmunk::plugin::shared_types;
wasmtime::component::bindgen!({
    path: "../plugins_api/wit/v_0.1.0/",
    world: "parser",
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
    async: {
        only_imports: [],
    },
});

impl<'a> From<&'a HostParseConfig> for ParserConfig<'a> {
    fn from(value: &'a HostParseConfig) -> Self {
        Self {
            place_holder_config: &value.placeholder,
        }
    }
}

impl From<InitError> for PluginGuestInitError {
    fn from(value: InitError) -> Self {
        use PluginGuestInitError as GuestErr;
        match value {
            InitError::Config(msg) => GuestErr::Config(msg),
            InitError::Io(msg) => GuestErr::IO(msg),
            InitError::Unsupported(msg) => GuestErr::Unsupported(msg),
            InitError::Other(msg) => GuestErr::Other(msg),
        }
    }
}
