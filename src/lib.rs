use std::{error as stderror};
use std::mem;
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
    type Externalities: ?Sized;
}

pub trait AsExternalities {
    type Externalities: ?Sized;

    fn as_externalities(&mut self) -> &mut Self::Externalities;
}

pub trait Backend: Sized {
    type Context: Context;
    type State: AsExternalities<Externalities=ExternalitiesOf<Self::Context>>;
    type Operation;
    type Error: stderror::Error + 'static;

    fn state_at(
        &self,
        hash: Option<HashOf<Self::Context>>
    ) -> Result<Self::State, Self::Error>;

    fn commit(
        &self,
        operation: Self::Operation,
    ) -> Result<(), Self::Error>;
}

pub trait Executor: Sized {
    type Context: Context;
    type Error: stderror::Error + 'static;

    fn execute_block(
        &self,
        block: &BlockOf<Self::Context>,
        state: &mut ExternalitiesOf<Self::Context>
    ) -> Result<(), Self::Error>;
}

pub struct ImportOperation<B: Backend> {
    pub block: BlockOf<B::Context>,
    pub state: B::State,
}

pub struct Operation<B: Backend> {
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

pub struct Chain<C: Context, B: Backend, E> {
    executor: E,
    backend: B,
    pending: Operation<B>,
    _marker: PhantomData<C>,
}

pub enum Error {
    Backend(Box<stderror::Error>),
    Executor(Box<stderror::Error>),
}

impl<C: Context, B, E> Chain<C, B, E> where
    B: Backend<Context=C, Operation=Operation<B>>,
    E: Executor<Context=C>,
{
    pub fn new(backend: B, executor: E) -> Self {
        Self {
            executor, backend,
            pending: Default::default(),
            _marker: Default::default(),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::error as stderror;
    use std::fmt;
    use std::sync::{Arc, RwLock};

    pub struct DummyBlock(usize);

    impl Block for DummyBlock {
        type Hash = usize;

        fn hash(&self) -> usize { self.0 }
        fn parent_hash(&self) -> Option<usize> { if self.0 == 0 { None } else { Some(self.0 - 1) } }
    }

    pub struct DummyBackendInner {
        blocks: HashMap<usize, DummyBlock>,
        head: usize,
    }

    pub type DummyBackend = RwLock<DummyBackendInner>;

    impl Backend for Arc<DummyBackend> {
        type Context = DummyContext;
        type State = DummyState;
        type Error = DummyError;
        type Operation = Operation<Self>;

        fn state_at(
            &self,
            _hash: Option<usize>
        ) -> Result<DummyState, DummyError> {
            let _ = self.read().expect("backend lock is poisoned");

            Ok(DummyState {
                _backend: self.clone()
            })
        }

        fn commit(
            &self,
            operation: Operation<Self>,
        ) -> Result<(), DummyError> {
            let mut this = self.write().expect("backend lock is poisoned");
            for block in operation.import_block {
                this.blocks.insert(block.block.0, block.block);
            }
            if let Some(head) = operation.set_head {
                this.head = head;
            }

            Ok(())
        }
    }

    pub struct DummyState {
        _backend: Arc<DummyBackend>,
    }

    pub trait DummyExternalities {
        fn test_fn(&self) -> usize { 42 }
    }

    impl DummyExternalities for DummyState { }

    impl AsExternalities for DummyState {
        type Externalities = dyn DummyExternalities + 'static;

        fn as_externalities(&mut self) -> &mut (dyn DummyExternalities + 'static) {
            self
        }
    }

    pub struct DummyContext;

    impl Context for DummyContext {
        type Block = DummyBlock;
        type Externalities = dyn DummyExternalities + 'static;
    }

    #[derive(Debug)]
    pub struct DummyError;

    impl fmt::Display for DummyError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            "dummy error".fmt(f)
        }
    }

    impl stderror::Error for DummyError { }

    pub struct DummyExecutor;

    impl Executor for Arc<DummyExecutor> {
        type Context = DummyContext;
        type Error = DummyError;

        fn execute_block(
            &self,
            _block: &DummyBlock,
            _state: &mut (dyn DummyExternalities + 'static),
        ) -> Result<(), DummyError> {
            Ok(())
        }
    }
}
