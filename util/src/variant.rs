use std::any::TypeId;
use std::marker::Unsize;
use std::ops::{CoerceUnsized, Deref, DerefMut};

/// Essentially equivalent to `Box<T>` in that this struct takes ownership of the value passed to it,
/// however this struct also records the type ID of the value it owns, and because of this allows safe
/// downcasting. When a `Variant<T>` is dropped, the pointer it holds is moved back into a box which
/// is immediately dropped, deallocating the memory on the heap.
pub struct Variant<T: ?Sized> {
    descriminant: TypeId,
    value: *mut T
}

// Since boxes are leveraged to construct this trait, and since boxes own the values they are constructed
// with, it is safe to implement these traits if T satisfies the traits respectively.
unsafe impl<T: ?Sized + Send> Send for Variant<T> { }
unsafe impl<T: ?Sized + Sync> Sync for Variant<T> { }

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Variant<U>> for Variant<T> { }

impl<T: 'static> Variant<T> {
    /// Allocate space on the heap and move the given value into that memory location. This function also
    /// stores the type ID of the given value.
    pub fn new(x: T) -> Self {
        Variant {
            descriminant: TypeId::of::<T>(),
            value: Box::into_raw(Box::new(x))
        }
    }
}

impl<T: ?Sized> Variant<T> {
    /// Attempt to downcast the stored value to the given new type, returning a shared reference to that value.
    /// If this operation can be completed safely, a reference is returned, else `None` is returned.
    pub fn downcast<C: 'static>(&self) -> Option<&C> {
        if TypeId::of::<C>() == self.descriminant {
            unsafe { Some(&*(self.value as *mut C)) }
        } else {
            None
        }
    }

    /// Attempt to downcast the stored value to the given new type, returning a mutable reference to that value.
    /// If this operation can be completed safely, a mutable reference is returned, else `None` is returned.
    pub fn downcast_mut<C: 'static>(&mut self) -> Option<&mut C> {
        if TypeId::of::<C>() == self.descriminant {
            unsafe { Some(&mut *(self.value as *mut C)) }
        } else {
            None
        }
    }
}

impl<T: ?Sized> Drop for Variant<T> {
    fn drop(&mut self) {
        // This is safe since we know that the pointer was allocated by a box via the `new` function
        unsafe { drop(Box::from_raw(self.value)); }
    }
}

impl<T: ?Sized> Deref for Variant<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.value }
    }
}

impl<T: ?Sized> DerefMut for Variant<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.value }
    }
}