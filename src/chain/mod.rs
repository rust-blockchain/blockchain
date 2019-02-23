mod importer;
mod block_builder;

pub use self::importer::Importer;
pub use self::block_builder::BlockBuilder;

use std::{fmt, error as stderror};
use crate::traits::{Backend, BaseContext, BlockOf, HashOf};

pub struct ImportOperation<C: BaseContext, B: Backend<C>> {
	pub block: BlockOf<C>,
	pub state: B::State,
}

pub struct Operation<C: BaseContext, B: Backend<C>> {
	pub import_block: Vec<ImportOperation<C, B>>,
	pub set_head: Option<HashOf<C>>,
}

impl<C: BaseContext, B> Default for Operation<C, B> where
	B: Backend<C>
{
	fn default() -> Self {
		Self {
			import_block: Vec::new(),
			set_head: None,
		}
	}
}

#[derive(Debug)]
pub enum Error {
	Backend(Box<stderror::Error>),
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
