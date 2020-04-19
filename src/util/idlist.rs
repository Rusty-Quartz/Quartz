use std::mem;
use std::slice::Iter;
use std::slice::IterMut;
use std::ops::{Index, IndexMut};

// Represents an object which has a usize as a modifiable ID
pub trait Identify {
    fn set_id(&mut self, id: usize);

    fn id(&self) -> usize;
}

// A map from IDs (i32) to an object of a given type. Internally, this operates on vectors
// and indexing so it is more efficient than a hash map.
pub struct IdList<T: Identify> {
    inner: Vec<Option<T>>,
    free_ids: Vec<usize>
}

impl<T: Identify> IdList<T> {
    // New empty list
    pub fn new() -> Self {
        IdList {
            inner: Vec::new(),
            free_ids: Vec::new()
        }
    }

    // New list with the given initial capacity
    pub fn with_capacity(capacity: usize) -> Self {
        IdList {
            inner: Vec::with_capacity(capacity),
            free_ids: Vec::new()
        }
    }

    // Get an iterator over this list that provides shared references
    pub fn iter(&self) -> IdListIterator<T, Iter<'_, Option<T>>> {
        IdListIterator {
            inner: self.inner.iter()
        }
    }

    // Get an iterator over this list that provides mutable references
    pub fn iter_mut(&mut self) -> IdListIteratorMut<T, IterMut<'_, Option<T>>> {
        IdListIteratorMut {
            inner: self.inner.iter_mut()
        }
    }

    // Adds the given item to this list, setting its ID to the next open ID in this list and returning that ID
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

    // Removes the item with the given ID returning that item if it exists
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
        pub struct $name<'a, T: 'a, I: Iterator<Item = $wrapped_itype>> {
            inner: I
        }
        
        impl<'a, T, I: Iterator<Item = $wrapped_itype>> Iterator for $name<'a, T, I> {
            type Item = $itype;
        
            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    match self.inner.next() {
                        Some(value) => {
                            match value {
                                Some(item) => return Some(item),
                                None => continue
                            }
                        },
                        None => return None
                    }
                }
            }
        }
    };
}

id_list_iter!(IdListIterator, &'a T, &'a Option<T>);
id_list_iter!(IdListIteratorMut, &'a mut T, &'a mut Option<T>);