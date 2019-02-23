pub trait Block {
    type Hash;

    fn hash(&self) -> Self::Hash;
    fn parent_hash(&self) -> Self::Hash;
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
    type Error;

    fn state_at(&self) -> Result<StateOf<Self::Context>, Self::Error>;
    fn commit(&mut self, operation: Operation<Self::Context>) -> Result<(), Self::Error>;
}

pub trait Executor {
    type Context: Context;
    type Error;

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

pub struct Chain<C: Context, B, E> {
    executor: E,
    backend: B,
    pending: Operation<C>,
}

pub enum Error {

}

impl<C: Context, B, E> Chain<C, B, E> where
    B: Backend<Context=C>,
    E: Executor<Context=C>,
{
    pub fn import_block(&mut self, block: BlockOf<C>) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn set_head(&mut self, head: HashOf<C>) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn discard(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
}
