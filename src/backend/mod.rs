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
use core::ops::{Deref, DerefMut};

/// Locked backend.
pub struct Locked<Ba> {
	backend: Ba,
	import_lock: Arc<Mutex<()>>,
}

impl<Ba> Locked<Ba> {
	/// Create a new locked backend.
	pub fn new(backend: Ba) -> Self {
		Self {
			backend,
			import_lock: Arc::new(Mutex::new(())),
		}
	}

	/// Lock import.
	pub fn lock_import(&self) -> MutexGuard<()> {
		self.import_lock.lock().expect("Lock is poisoned")
	}
}

impl<Ba: Clone> Clone for Locked<Ba> {
	fn clone(&self) -> Self {
		Self {
			backend: self.backend.clone(),
			import_lock: self.import_lock.clone(),
		}
	}
}

impl<Ba> Deref for Locked<Ba> {
	type Target = Ba;

	fn deref(&self) -> &Ba {
		&self.backend
	}
}

impl<Ba> DerefMut for Locked<Ba> {
	fn deref_mut(&mut self) -> &mut Ba {
		&mut self.backend
	}
}
