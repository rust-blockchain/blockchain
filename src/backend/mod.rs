//! Basic backend definitions and memory backend.

mod memory;
mod route;

pub use self::memory::{MemoryState, MemoryBackend};
pub use self::route::{tree_route, TreeRoute};
