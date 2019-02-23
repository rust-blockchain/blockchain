use std::{error as stderror};
use std::mem;

pub trait Block {
    type Hash;

    fn hash(&self) -> Self::Hash;
    fn parent_hash(&self) -> Option<Self::Hash>;
}

pub type StateOf<C> = <C as Context>::State;
pub type BlockOf<C> = <C as Context>::Block;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;

pub trait Context {
    type Block: Block;
    type State;
}

pub trait Backend {
    type Context: Context;
    type Error: stderror::Error + 'static;

    fn state_at(
        &self,
        hash: Option<HashOf<Self::Context>>
    ) -> Result<StateOf<Self::Context>, Self::Error>;

    fn commit(
        &mut self,
        operation: Operation<Self::Context>
    ) -> Result<(), Self::Error>;
}

pub trait Executor {
    type Context: Context;
    type Error: stderror::Error + 'static;

    fn execute_block(
        &self,
        block: &BlockOf<Self::Context>,
        state: &mut StateOf<Self::Context>
    ) -> Result<(), Self::Error>;
}

pub struct ImportOperation<C: Context> {
    pub block: BlockOf<C>,
    pub state: StateOf<C>,
}

pub struct Operation<C: Context> {
    pub import_block: Vec<ImportOperation<C>>,
    pub set_head: Option<HashOf<C>>,
}

impl<C: Context> Default for Operation<C> {
    fn default() -> Self {
        Self {
            import_block: Vec::new(),
            set_head: None,
        }
    }
}

pub struct Chain<C: Context, B, E> {
    executor: E,
    backend: B,
    pending: Operation<C>,
}

pub enum Error {
    Backend(Box<stderror::Error>),
    Executor(Box<stderror::Error>),
}

impl<C: Context, B, E> Chain<C, B, E> where
    B: Backend<Context=C>,
    E: Executor<Context=C>,
{
    pub fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Error> {
        let mut state = self.backend.state_at(block.parent_hash())
            .map_err(|e| Error::Backend(Box::new(e)))?;
        self.executor.execute_block(&block, &mut state)
            .map_err(|e| Error::Executor(Box::new(e)))?;

        let operation = ImportOperation { block, state };
        self.pending.import_block.push(operation);

        Ok(())
    }

    pub fn set_head(&mut self, head: HashOf<C>) -> Result<(), Error> {
        self.pending.set_head = Some(head);

        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        let mut operation = Operation::default();
        mem::swap(&mut operation, &mut self.pending);

        self.backend.commit(operation)
            .map_err(|e| Error::Backend(Box::new(e)))?;

        Ok(())
    }

    pub fn discard(&mut self) -> Result<(), Error> {
        self.pending = Operation::default();

        Ok(())
    }
}
