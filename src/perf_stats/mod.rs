#[cfg(feature = "perf_stats")]
mod real_impl;
#[cfg(feature = "perf_stats")]
pub use real_impl::{start_tile,finish_tile,measure,PerfStats};

#[cfg(not(feature = "perf_stats"))]
mod dummy_impl;
#[cfg(not(feature = "perf_stats"))]
pub use dummy_impl::{start_tile,finish_tile,measure,PerfStats};