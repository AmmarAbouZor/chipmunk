use wasmtime_wasi::{ResourceTable, WasiCtx, WasiView};

use super::binding::{ParseError, ParseReturn};

pub struct PluginParserState {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
    pub results_queue: Vec<Result<ParseReturn, ParseError>>,
}

impl PluginParserState {
    pub fn new(ctx: WasiCtx, table: ResourceTable) -> Self {
        Self {
            ctx,
            table,
            results_queue: Default::default(),
        }
    }
}

impl WasiView for PluginParserState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}
