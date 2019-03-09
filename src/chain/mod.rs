//! Chain importer and block builder.

mod importer;
mod block_builder;

pub use self::importer::{SharedBackend, Importer};
pub use self::block_builder::BlockBuilder;

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
		match self {
			Error::Backend(_) => "Backend failure".fmt(f)?,
			Error::Executor(_) => "Executor failure".fmt(f)?,
			Error::IsGenesis => "Block is genesis block and cannot be imported".fmt(f)?,
			Error::ParentNotFound => "Parent block cannot be found".fmt(f)?,
		}

		Ok(())
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
