// canvasx-runtime/src/ipc/mod.rs
//
// Generic IPC bridge — connects to any host application via Windows named pipes.
// The protocol is JSON-based: {ns, cmd, args} → {ok, data, error}.
// The pipe name and polling behaviour are fully configurable.
//
// The `sentinel` sub-module provides a high-level Sentinel-specific bridge.

pub mod client;
pub mod protocol;
pub mod sentinel;
