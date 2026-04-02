#[allow(
    clippy::all,
    warnings,
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code
)]
mod perf_bindings {
    include!(concat!(env!("OUT_DIR"), "/perf_bindings.rs"));
}
pub use perf_bindings::*;
