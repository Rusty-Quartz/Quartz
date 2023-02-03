use std::{collections::HashMap, sync::Arc};

use flashmap::{new as new_flashmap, ReadGuard, ReadHandle, View, WriteHandle};

use crate::{UlnStr, UnlocalizedName};

pub struct Registry<T> {
    read_handle: ReadHandle<UnlocalizedName, Arc<T>>,
    write_handle: WriteHandle<UnlocalizedName, Arc<T>>,
}

impl<T> Registry<T> {
    fn empty() -> Self {
        let (write, read) = new_flashmap();
        Registry {
            read_handle: read,
            write_handle: write,
        }
    }

    #[allow(unused)]
    fn new(map: impl IntoIterator<Item = (UnlocalizedName, T)>) -> Registry<T> {
        let (mut write, read) = new_flashmap();

        let mut write_guard = write.guard();

        for (key, val) in map {
            write_guard.insert(key, Arc::new(val));
        }

        drop(write_guard);
        Registry {
            read_handle: read,
            write_handle: write,
        }
    }

    /// Returns a read handle, which allows the creation of multiple read guards to the registry
    pub fn get_read_handle(&self) -> RegistryHandle<T> {
        RegistryHandle {
            read_handle: self.read_handle.clone(),
        }
    }

    /// Inserts the key value pair into the registry
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn insert(&mut self, key: UnlocalizedName, value: T) {
        let mut write_guard = self.write_handle.guard();

        write_guard.insert(key, Arc::new(value));
    }

    /// Inserts all the key value pair into the registry
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn insert_all(&mut self, entries: impl IntoIterator<Item = (UnlocalizedName, T)>) {
        let mut write_guard = self.write_handle.guard();

        for (key, value) in entries {
            write_guard.insert(key, Arc::new(value));
        }
    }

    /// Clears the registry
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn clear(&mut self) {
        let mut write_guard = self.write_handle.guard();

        // This kinda sucks, but this method is not going to be used much if at all
        let keys = write_guard.keys().cloned().collect::<Vec<_>>();

        for key in keys {
            write_guard.remove(key);
        }
    }

    /// Replaces the current data with the data stored in `map`
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn replace_map(&mut self, map: HashMap<UnlocalizedName, T>) {
        let mut write_guard = self.write_handle.guard();

        // I don't know if the filter call is slower than just cloning the whole map
        // I think hashing UnlocalizedName is faster than allocating mem but im not sure
        let to_remove = write_guard
            .keys()
            .filter(|k| !map.contains_key(*k))
            .cloned()
            .collect::<Vec<_>>();

        for key in to_remove {
            write_guard.remove(key);
        }

        for (key, value) in map {
            write_guard.insert(key, Arc::new(value));
        }
    }
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Registry::<T>::empty()
    }
}

/// A handle to a Registry, allows the creation of read guards to read a snapshot of the data in the registry   
pub struct RegistryHandle<T> {
    read_handle: ReadHandle<UnlocalizedName, Arc<T>>,
}

impl<T> RegistryHandle<T> {
    pub fn read(&self) -> View<ReadGuard<'_, UnlocalizedName, Arc<T>>> {
        self.read_handle.guard()
    }
}


pub trait Resolver<T> {
    fn resolve_entry(&self, entry: &UlnStr) -> Option<Arc<T>>;
}

impl<T> Resolver<T> for RegistryHandle<T> {
    fn resolve_entry(&self, entry: &UlnStr) -> Option<Arc<T>> {
        let guard = self.read();
        guard.get(entry).cloned()
    }
}

impl<'a, T> Resolver<T> for View<ReadGuard<'a, UnlocalizedName, Arc<T>>> {
    fn resolve_entry(&self, entry: &UlnStr) -> Option<Arc<T>> {
        self.get(entry).cloned()
    }
}

#[derive(Clone)]
pub enum Resolvable<T> {
    Unresolved(UnlocalizedName),
    Resolved(Arc<T>),
}

impl<T> Resolvable<T> {
    pub fn resolve<R: Resolver<T>>(&mut self, resolver: &R) -> Option<Arc<T>> {
        match self {
            Self::Unresolved(uln) =>
                if let Some(entry) = resolver.resolve_entry(uln) {
                    *self = Resolvable::Resolved(entry.clone());
                    Some(entry)
                } else {
                    None
                },
            Self::Resolved(t) => Some(t.clone()),
        }
    }

    pub fn get(&self) -> Option<Arc<T>> {
        match self {
            Self::Unresolved(_uln) => None,
            Self::Resolved(t) => Some(t.clone()),
        }
    }
}
