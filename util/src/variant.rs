use std::any::{Any, TypeId};

/// Downcasts the given shared reference to the new type. If the type cannot be safely downcasted, then
/// `None` is returned.
///
/// # Examples
///
/// ```
/// # use util::variant::downcast_ref;
/// use std::any::Any;
///
/// fn test<T: Any + ToString>(shared: &T) {
///     assert_eq!(shared.to_string(), "10");
///     assert_eq!(*downcast_ref::<_, i32>(shared).unwrap(), 10);
///     assert!(downcast_ref::<_, f32>(shared).is_none());
/// }
///
/// let x: i32 = 10;
/// test(&x);
/// ```
#[inline]
pub fn downcast_ref<T: Any + ?Sized, U: 'static>(x: &T) -> Option<&U> {
    if x.type_id() == TypeId::of::<U>() {
        Some(unsafe { &*(x as *const T as *const U) })
    } else {
        None
    }
}

/// Downcasts the given mutable reference to the new type. If the type cannot be safely downcasted, then
/// `None` is returned.
///
/// # Examples
///
/// ```
/// # use util::variant::downcast_mut;
/// use std::any::Any;
///
/// fn test<T: Any + ToString>(mutable: &mut T) {
///     assert_eq!(mutable.to_string(), "10");
///     *downcast_mut::<_, i32>(&mut *mutable).unwrap() += 5;
///     assert_eq!(mutable.to_string(), "15");
///
///     assert!(downcast_mut::<_, f32>(mutable).is_none());
/// }
///
/// let mut x: i32 = 10;
/// test(&mut x);
/// ```
#[inline]
pub fn downcast_mut<T: Any + ?Sized, U: 'static>(x: &mut T) -> Option<&mut U> {
    if (*x).type_id() == TypeId::of::<U>() {
        Some(unsafe { &mut *(x as *mut T as *mut U) })
    } else {
        None
    }
}
