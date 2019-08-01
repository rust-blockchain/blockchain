use std::error as stderror;
use crate::Block;

/// Trait used for committing blocks, usually built on top of a backend.
pub trait BlockImporter {
	/// Block type
	type Block: Block;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit a block into the backend, and handle consensus and auxiliary.
	fn import_block(&mut self, block: Self::Block) -> Result<(), Self::Error>;
}

/// Shared block importer.
pub trait SharedBlockImporter: BlockImporter + Clone {
	/// Commit a block into the backend, and handle consensus and auxiliary.
	fn import_block(&self, block: Self::Block) -> Result<(), Self::Error>;
}

/// Trait used for committing prebuilt blocks, usually built on top of a backend.
pub trait RawImporter {
	/// Operation type
	type Operation;
	/// Error type
	type Error: stderror::Error + 'static;

	/// Commit a prebuilt block into the backend, and handle consensus and auxiliary.
	fn import_raw(
		&mut self,
		operation: Self::Operation
	) -> Result<(), Self::Error>;
}

/// Shared raw importer.
pub trait SharedRawImporter: RawImporter + Clone {
	/// Commit a prebuilt block into the backend, and handle consensus and
	/// auxiliary.
	fn import_raw(
		&self,
		operation: Self::Operation
	) -> Result<(), Self::Error>;
}
