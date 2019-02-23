use std::{error as stderror};
use std::mem;
use std::sync::Arc;
use std::marker::PhantomData;

pub trait Block {
    type Hash;

    fn hash(&self) -> Self::Hash;
    fn parent_hash(&self) -> Option<Self::Hash>;
}

pub type ExternalitiesOf<C> = <C as Context>::Externalities;
pub type BlockOf<C> = <C as Context>::Block;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;

pub trait Context {
    type Block: Block;
    type Externalities;
}

pub trait AsExternalities {
    type Externalities;

    fn as_externalities(&mut self) -> &mut Self::Externalities;
}

pub trait Backend {
    type Context: Context;
    type State: AsExternalities<Externalities=ExternalitiesOf<Self::Context>>;
    type Error: stderror::Error + 'static;

    fn state_at(
        &self,
        hash: Option<HashOf<Self::Context>>
    ) -> Result<Self::State, Self::Error>;

    fn commit(
        &self,
        operation: Operation<Self>
    ) -> Result<(), Self::Error>;
}

pub trait Executor {
    type Context: Context;
    type Error: stderror::Error + 'static;

    fn execute_block(
        &self,
        block: &BlockOf<Self::Context>,
        state: &mut ExternalitiesOf<Self::Context>
    ) -> Result<(), Self::Error>;
}

pub struct ImportOperation<B: Backend + ?Sized> {
    pub block: BlockOf<B::Context>,
    pub state: B::State,
}

pub struct Operation<B: Backend + ?Sized> {
    pub import_block: Vec<ImportOperation<B>>,
    pub set_head: Option<HashOf<B::Context>>,
}

impl<B: Backend> Default for Operation<B> {
    fn default() -> Self {
        Self {
            import_block: Vec::new(),
            set_head: None,
        }
    }
}

pub struct Chain<C: Context, B: Backend + ?Sized, E> {
    executor: Arc<E>,
    backend: Arc<B>,
    pending: Operation<B>,
    _marker: PhantomData<C>,
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
        self.executor.execute_block(&block, state.as_externalities())
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
