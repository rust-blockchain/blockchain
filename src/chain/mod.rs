//! Chain importer and block builder.

mod importer;
mod block_builder;

pub use self::importer::{SharedBackend, Importer};
pub use self::block_builder::BlockBuilder;

use crate::traits::ImportBlock;
use std::sync::{Arc, Mutex};
use std::{fmt, error as stderror};

/// Error type for chain.
#[derive(Debug)]
pub enum Error {
	/// Backend error.
	Backend(Box<stderror::Error>),
	/// Executor error.
	Executor(Box<stderror::Error>),
	/// Block is genesis block and cannot be imported.
	IsGenesis,
	/// Parent is not in the backend so block cannot be imported.
	ParentNotFound,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl stderror::Error for Error {
	fn source(&self) -> Option<&(dyn stderror::Error + 'static)> {
		match self {
			Error::Backend(e) => Some(e.as_ref()),
			Error::Executor(e) => Some(e.as_ref()),
			Error::IsGenesis | Error::ParentNotFound => None,
		}
	}
}

/// An import block that can be shared across threads.
pub struct SharedImportBlock<I: ImportBlock> {
	import_block: Arc<Mutex<I>>,
}

impl<I: ImportBlock> SharedImportBlock<I> {
	/// Create a new shared import block.
	pub fn new(import_block: I) -> Self {
		Self {
			import_block: Arc::new(Mutex::new(import_block)),
		}
	}
}

impl<I: ImportBlock> Clone for SharedImportBlock<I> {
	fn clone(&self) -> Self {
		Self {
			import_block: self.import_block.clone(),
		}
	}
}

impl<I: ImportBlock> ImportBlock for SharedImportBlock<I> {
	type Block = I::Block;
	type Error = I::Error;

	fn import_block(&mut self, block: Self::Block) -> Result<(), Self::Error> {
		self.import_block.lock().expect("Lock is poisoned")
			.import_block(block)
	}
}
