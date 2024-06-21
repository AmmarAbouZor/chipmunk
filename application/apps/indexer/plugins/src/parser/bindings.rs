//TODO AAZ: Revisit ownership option when benchmarks are implemented
wasmtime::component::bindgen!({
    path: "../plugins_api/wit/v_0.1.0/",
    world: "parser",
    // ownership: Borrowing {
    //     duplicate_if_necessary: false
    // },
    async: {
        only_imports: [],
    },
});
