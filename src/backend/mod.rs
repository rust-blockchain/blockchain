//! Basic backend definitions and memory backend.

mod memory;
mod route;
mod traits;
mod operation;
mod state;

pub use self::memory::{MemoryBackend, MemoryDatabase, SharedMemoryBackend, Error as MemoryError};
pub use self::route::{tree_route, TreeRoute};
pub use self::operation::{BlockData, ImportOperation, Operation};
pub use self::traits::{Store, ChainQuery, ChainSettlement, OperationError, Committable, SharedCommittable};
pub use self::state::KeyValueMemoryState;

use std::sync::{Arc, Mutex, MutexGuard};

/// Standalone import lock.
pub struct ImportLock(Arc<Mutex<()>>);

impl Clone for ImportLock {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl ImportLock {
	/// Create a new import lock.
	pub fn new() -> Self {
		Self(Arc::new(Mutex::new(())))
	}

	/// Lock the import.
	pub fn lock(&self) -> MutexGuard<()> {
		self.0.lock().expect("Lock is poisoned")
	}
}
