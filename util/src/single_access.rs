use std::{
    cell::{Cell, UnsafeCell},
    marker::Unsize,
    ops::{CoerceUnsized, Deref, DerefMut},
    ptr,
};

/// Similar to [`SingleAccessorBox`], except that this structure allocates its value on the stack.
///
/// [`SingleAccessorBox`]: crate::single_access::SingleAccessorBox
pub struct SingleAccessor<T> {
    value: UnsafeCell<T>,
    taken: Cell<bool>,
}

unsafe impl<T: Send> Send for SingleAccessor<T> {}

impl<T> SingleAccessor<T> {
    /// Creates a new `SingleAccessor` with the given value.
    #[inline]
    pub const fn new(value: T) -> Self {
        SingleAccessor {
            value: UnsafeCell::new(value),
            taken: Cell::new(false),
        }
    }

    /// Attempts to take exclusive access to the data guarded by this structure. If this fails
    /// because access is already taken, then `None` is returned.
    #[inline]
    pub fn take(&self) -> Option<AccessGuard<'_, T>> {
        if self.taken.replace(true) {
            return None;
        }

        Some(AccessGuard {
            value: self.value.get(),
            flag: &self.taken,
        })
    }
}

/// A smart pointer created by the [`take`] method of [`SingleAccessor`] to enforce its access constraints.
///
/// [`SingleAccessor`]: crate::single_access::SingleAccessorBox
/// [`take`]: crate::single_access::SingleAccessor::take
pub struct AccessGuard<'a, T> {
    value: *mut T,
    flag: &'a Cell<bool>,
}

impl<'a, T> Drop for AccessGuard<'a, T> {
    fn drop(&mut self) {
        self.flag.set(false);
    }
}

impl<'a, T> Deref for AccessGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value }
    }
}

impl<'a, T> DerefMut for AccessGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.value }
    }
}

/// Acts the same as a box in terms of memory management and provides an interface allowing for interior
/// mutability.
///
/// This type, unlike [`RefCell`], only allows one reference to its data at a time, and that reference is
/// always mutable. In fact, when [`take`] is called on a single accessor box, its internal pointer is set to
/// null until the smart pointer is dropped, at which point it is replaced.
///
/// [`RefCell`]: std::cell::RefCell
/// [`take`]: crate::single_access::SingleAccessorBox::take
// TODO: Delete if unused
#[repr(transparent)]
pub struct SingleAccessorBox<T: ?Sized> {
    value: Cell<*mut T>,
}

unsafe impl<T: ?Sized + Send> Send for SingleAccessorBox<T> {}
impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<SingleAccessorBox<U>>
    for SingleAccessorBox<T>
{
}

impl<T> SingleAccessorBox<T> {
    /// Allocates memory on the heap and places `x` into it. Allocation is skipped if `T` is a ZST.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::single_access::SingleAccessorBox;
    /// let pi = SingleAccessorBox::new(3.141592653_f32);
    /// ```
    #[inline]
    pub fn new(x: T) -> Self {
        SingleAccessorBox {
            value: Cell::new(Box::into_raw(Box::new(x))),
        }
    }
}

impl<T: ?Sized> SingleAccessorBox<T> {
    /// Attempts to take the value stored in this box, returning exclusive access to that value, or `None`
    /// if the value is already taken.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::single_access::SingleAccessorBox;
    /// let x = SingleAccessorBox::new(5_i32);
    /// let mut guard = x.take();
    ///
    /// // We can take it once
    /// assert!(guard.is_some());
    /// // But not twice
    /// assert!(x.take().is_none());
    ///
    /// // Modify the value and release our access
    /// *guard.unwrap() += 5;
    ///
    /// assert_eq!(x.take().as_mut().map(|guard| **guard), Some(10_i32));
    /// ```
    #[inline]
    pub fn take(&self) -> Option<BoxAccessGuard<'_, T>> {
        BoxAccessGuard::new(&self.value)
    }
}

impl<T: ?Sized> Drop for SingleAccessorBox<T> {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(self.value.get()));
        }
    }
}

/// A smart pointer created by the [`take`] method of [`SingleAccessorBox`] to enforce its access constraints.
///
/// [`SingleAccessorBox`]: crate::single_access::SingleAccessorBox
/// [`take`]: crate::single_access::SingleAccessorBox::take
pub struct BoxAccessGuard<'a, T: ?Sized> {
    value: *mut T,
    source: &'a Cell<*mut T>,
}

impl<'a, T: ?Sized> BoxAccessGuard<'a, T> {
    /// Creates a new access-guard smart pointer with the given cell.
    ///
    /// If the pointer in the cell is null, then `None` is returned, otherwise the data part of the pointer
    /// is set to null and the reference is constructed and returned.
    #[inline]
    fn new(source: &'a Cell<*mut T>) -> Option<Self> {
        let value = source.get();

        // Ensure the value hasn't already been taken
        if value.is_null() {
            return None;
        }

        // Set the data part of the pointer in the cell to null
        source.set(value.set_ptr_value(ptr::null_mut()));

        Some(BoxAccessGuard { value, source })
    }
}

impl<'a, T: ?Sized> Drop for BoxAccessGuard<'a, T> {
    fn drop(&mut self) {
        self.source.set(self.value);
    }
}

impl<'a, T: ?Sized> Deref for BoxAccessGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.value }
    }
}

impl<'a, T: ?Sized> DerefMut for BoxAccessGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.value }
    }
}
