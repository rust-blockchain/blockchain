//! Common trait definitions related to block context.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod traits;
pub use crate::traits::*;
