#[cfg(feature = "perf-stats")]
mod real_impl;
#[cfg(feature = "perf-stats")]
pub use real_impl::{start_tile,finish_tile,measure,PerfStats};

#[cfg(not(feature = "perf-stats"))]
mod dummy_impl;
#[cfg(not(feature = "perf-stats"))]
pub use dummy_impl::{start_tile,finish_tile,measure,PerfStats};