use std::collections::HashMap;
use std::error as stderror;
use core::convert::Infallible;
use crate::StorageExternalities;

/// State stored in memory.
#[derive(Clone, Default)]
pub struct KeyValueMemoryState {
	storage: HashMap<Vec<u8>, Vec<u8>>,
}

impl AsRef<HashMap<Vec<u8>, Vec<u8>>> for KeyValueMemoryState {
	fn as_ref(&self) -> &HashMap<Vec<u8>, Vec<u8>> {
		&self.storage
	}
}

impl AsMut<HashMap<Vec<u8>, Vec<u8>>> for KeyValueMemoryState {
	fn as_mut(&mut self) -> &mut HashMap<Vec<u8>, Vec<u8>> {
		&mut self.storage
	}
}

impl StorageExternalities<Infallible> for KeyValueMemoryState {
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Infallible> {
		Ok(self.storage.get(key).map(|value| value.to_vec()))
	}

	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
		self.storage.insert(key, value);
	}

	fn remove_storage(&mut self, key: &[u8]) {
		self.storage.remove(key);
	}
}

impl StorageExternalities<Box<dyn stderror::Error>> for KeyValueMemoryState {
	fn read_storage(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Box<dyn stderror::Error>> {
		Ok(self.storage.get(key).map(|value| value.to_vec()))
	}

	fn write_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
		self.storage.insert(key, value);
	}

	fn remove_storage(&mut self, key: &[u8]) {
		self.storage.remove(key);
	}
}
