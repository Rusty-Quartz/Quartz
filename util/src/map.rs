use std::{
    mem,
    ops::{Index, IndexMut},
    slice::{Iter, IterMut},
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
    pub fn iter(&self) -> IdListIterator<T, Iter<'_, Option<T>>> {
        IdListIterator {
            inner: self.inner.iter(),
        }
    }

    /// Returns an iterator over mutable references to the values in this ID list.
    pub fn iter_mut(&mut self) -> IdListIteratorMut<T, IterMut<'_, Option<T>>> {
        IdListIteratorMut {
            inner: self.inner.iter_mut(),
        }
    }

    /// Adds the given item to this list, setting its ID to the next open ID in this list and returning that ID.
    pub fn add(&mut self, mut item: T) -> usize {
        if self.free_ids.is_empty() {
            let id = self.inner.len();
            item.set_id(id);
            self.inner.push(Some(item));
            id
        } else {
            let id = self.free_ids.pop().unwrap();
            item.set_id(id);
            self.inner[id] = Some(item);
            id
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

// Creates an iterator that essentially performs the flatten operation on another iterator
macro_rules! id_list_iter {
    ($name:ident, $itype:ty, $wrapped_itype:ty) => {
        #[doc = "A custom iterator over an ID list which skips over empty elements in the ID \
         list's internal vec."]
        pub struct $name<'a, T: 'a, I: Iterator<Item = $wrapped_itype>> {
            inner: I,
        }

        impl<'a, T, I: Iterator<Item = $wrapped_itype>> Iterator for $name<'a, T, I> {
            type Item = $itype;

            fn next(&mut self) -> Option<Self::Item> {
                while let Some(value) = self.inner.next() {
                    match value {
                        Some(item) => return Some(item),
                        None => continue,
                    }
                }

                None
            }
        }
    };
}

id_list_iter!(IdListIterator, &'a T, &'a Option<T>);
id_list_iter!(IdListIteratorMut, &'a mut T, &'a mut Option<T>);
