pub trait Executor {
    type State;

    fn dispatch(&self, ext: &mut Self::State);
}

pub trait Block {
    type Hash;

    fn hash(&self) -> Self::Hash;
    fn parent_hash(&self) -> Self::Hash;
}

pub type ExecutorOf<C> = <C as Context>::Executor;
pub type BlockOf<C> = <C as Context>::Block;
pub type StateOf<C> = <<C as Context>::Executor as Executor>::State;
pub type HashOf<C> = <BlockOf<C> as Block>::Hash;

pub trait Context {
    type Block: Block;
    type Executor: Executor;
}

pub trait Backend {
    type Context: Context;
}

pub struct ImportOperation<C: Context> {
    pub block: BlockOf<C>,
    pub state: StateOf<C>,
}

pub struct Operation<C: Context> {
    pub import_block: Option<ImportOperation<C>>,
    pub set_head: Option<HashOf<C>>,
}

pub struct Chain<C: Context, B> {
    executor: ExecutorOf<C>,
    backend: B,
}
