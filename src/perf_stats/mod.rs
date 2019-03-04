#[cfg(feature = "perf-stats")]
mod real_impl;
#[cfg(feature = "perf-stats")]
pub use real_impl::{finish_tile, measure, start_tile, PerfStats};

#[cfg(not(feature = "perf-stats"))]
mod dummy_impl;
#[cfg(not(feature = "perf-stats"))]
pub use dummy_impl::{finish_tile, measure, start_tile, PerfStats};
