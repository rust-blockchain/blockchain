# Rust Blockchain

*Rust Blockchain* is an unopinioned blockchain framework that helps
you to develop a blockchain project.

## Chain

The `chain` module handles block import and state storage. Assumptions
we have in this module:

* We have `Block`, which consists of a hash, and has a parent
  block. It forms a chain.
* At each `Block` there is a corresponding `State`.
* An executor that takes a block, and parent block's state. Executing
  it should get the current block's state.
