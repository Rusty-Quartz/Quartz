use std::{
    mem,
    ops::{Index, IndexMut},
};

/// Represents an object which has a usize as a modifiable ID.
pub trait Identify {
    /// Updates the ID of this object to the given new ID.
    fn set_id(&mut self, id: usize);

    /// Returns the ID of this object.
    fn id(&self) -> usize;
}

/// A map from IDs (usizes) to an object of a given type. Internally, this operates on vectors
/// and indexing so it is more efficient than a hash map.
pub struct IdList<T: Identify> {
    inner: Vec<Option<T>>,
    free_ids: Vec<usize>,
}

impl<T: Identify> IdList<T> {
    /// Returns a new, empty ID list with an empty internal vector.
    pub fn new() -> Self {
        IdList {
            inner: Vec::new(),
            free_ids: Vec::new(),
        }
    }

    /// Returns a new ID list with an internal vector with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        IdList {
            inner: Vec::with_capacity(capacity),
            free_ids: Vec::new(),
        }
    }

    /// Returns an iterator over shared references to the values in this ID list.
    pub fn iter(&self) -> impl Iterator<Item = &'_ T> {
        self.inner.iter().flatten()
    }

    /// Returns an iterator over mutable references to the values in this ID list.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &'_ mut T> {
        self.inner.iter_mut().flatten()
    }

    /// Adds the given item to this list, setting its ID to the next open ID in this list and returning that ID.
    pub fn insert(&mut self, mut item: T) -> usize {
        match self.free_ids.pop() {
            Some(id) => {
                item.set_id(id);
                self.inner[id] = Some(item);
                id
            }
            None => {
                let id = self.inner.len();
                item.set_id(id);
                self.inner.push(Some(item));
                id
            }
        }
    }

    /// Returns a shared reference to the element with the given ID, or `None` if no element has the given ID.
    pub fn get(&self, id: usize) -> Option<&T> {
        self.inner.get(id)?.as_ref()
    }

    /// Returns a mutable reference to the element with the given ID, or `None` if no element has the given ID.
    pub fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        self.inner.get_mut(id)?.as_mut()
    }

    /// Removes the item with the given ID returning that item if it exists, or None if it does not.
    pub fn remove(&mut self, id: usize) -> Option<T> {
        if id >= self.inner.len() {
            return None;
        }

        self.free_ids.push(id);
        // Swaps out the value and returns the old value
        mem::replace(&mut self.inner[id], None)
    }
}

impl<T: Identify> Index<usize> for IdList<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.inner[index].as_ref().unwrap()
    }
}

impl<T: Identify> IndexMut<usize> for IdList<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.inner[index].as_mut().unwrap()
    }
}
