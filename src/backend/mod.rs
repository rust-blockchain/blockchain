//! Basic backend definitions and memory backend.

mod memory;
mod route;

pub use self::memory::{KeyValueMemoryState, MemoryBackend, MemoryLikeBackend};
pub use self::route::{tree_route, TreeRoute};
