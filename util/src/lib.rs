#![warn(missing_docs)]
#![feature(coerce_unsized, unsize, set_ptr_value, test)]

//! Provides generic utilities for quartz, the minecraft server implementation in rust.

/// Defines a fast hasher for numeric types.
pub mod hash;
/// Configures log4rs to copy minecraft's logging style.
pub mod logging;
/// Contains optimized maps where hash maps are insufficient.
pub mod map;
/// Contains fast math utilities.
pub mod math;
/// Contains an implementation of a single-access box allowing for interior mutability.
pub mod single_access;
/// An implementation of Minecraft's unlocalized name.
// pub mod uln;
/// Allows for downcasting of trait types.
pub mod variant;

#[cfg(test)]
mod tests {
    #[cfg(not(debug_assertions))]
    extern crate test;
    #[cfg(not(debug_assertions))]
    use test::{black_box, Bencher};

    use super::*;
    use map::{IdList, Identify};

    struct Identifiable {
        id: usize,
        value: i32,
    }

    impl Identifiable {
        fn new(value: i32) -> Self {
            Identifiable { id: 0, value }
        }
    }

    impl Identify for Identifiable {
        fn set_id(&mut self, id: usize) {
            self.id = id;
        }

        fn id(&self) -> usize {
            self.id
        }
    }

    #[test]
    fn id_list_test() {
        let value1 = Identifiable::new(1);
        let value2 = Identifiable::new(2);
        let value3 = Identifiable::new(3);

        let mut id_list: IdList<Identifiable> = IdList::with_capacity(5);
        assert_eq!(id_list.insert(value1), 0, "Incorrect ID assigned.");
        assert_eq!(
            id_list.insert(value2),
            1,
            "Incorrect ID assigned (value 2)."
        );
        assert_eq!(
            id_list.insert(value3),
            2,
            "Incorrect ID assigned (value 3)."
        );
        assert!(id_list.get(1).is_some(), "ID lookup failed.");
        let value2 = id_list.remove(1);
        assert!(
            value2.is_some(),
            "Element removal failed: no value returned."
        );
        assert!(
            id_list.get(1).is_none(),
            "Element removal failed: value remained in list."
        );

        let value4 = Identifiable::new(4);

        assert_eq!(
            id_list.insert(value2.unwrap()),
            1,
            "Incorrect ID assigned after element removal."
        );
        assert_eq!(
            id_list.insert(value4),
            3,
            "Incorrect ID assigned after element removal and readdition."
        );

        id_list.remove(0);
        id_list.remove(2);

        let mut count: usize = 0;

        for element in id_list.iter() {
            assert_eq!(
                count,
                (element.id() - 1) / 2,
                "Element ID mismatch in Iter."
            );
            count += 1;
        }

        assert_eq!(
            count, 2,
            "IdList iterator did not cover the correct number of elements."
        );

        count = 0;
        for element in id_list.iter_mut() {
            element.value /= 4;
            assert_eq!(
                element.value as usize, count,
                "Element order incorrect in IterMut."
            );
            count += 1;
        }

        assert_eq!(
            count, 2,
            "IdList iterator mut did not cover the correct number of elements."
        );
    }

    #[bench]
    #[cfg(not(debug_assertions))]
    fn refcell(bencher: &mut Bencher) {
        use std::cell::RefCell;

        let cell1 = RefCell::new(0_i32);
        let cell2 = RefCell::new(0_i32);

        bencher.iter(move || {
            for _ in 0 .. 1000 {
                let mut ref1 = cell1.borrow_mut();
                *ref1 += 1;
                assert!(cell1.try_borrow_mut().is_err());
                *cell2.borrow_mut() -= *ref1;
                *ref1 += *cell2.borrow();
            }
        });
    }

    #[bench]
    #[cfg(not(debug_assertions))]
    fn single_accessor(bencher: &mut Bencher) {
        use single_access::SingleAccessor;

        let sa1 = SingleAccessor::new(0_i32);
        let sa2 = SingleAccessor::new(0_i32);

        bencher.iter(move || {
            for _ in 0 .. 1000 {
                let mut locmov1 = sa1.take().unwrap();
                *locmov1 += 1;
                assert!(sa1.take().is_none());
                *sa2.take().unwrap() -= *locmov1;
                *locmov1 += *sa2.take().unwrap();
            }
        });
    }

    #[bench]
    #[cfg(not(debug_assertions))]
    fn fast_inv_sqrt64(bencher: &mut Bencher) {
        use math::fast_inv_sqrt64;

        let mut x = 0.01f64;

        bencher.iter(move || {
            for _ in 0 .. 10000 {
                black_box(x * fast_inv_sqrt64(x));
                x += 0.01;
            }
        });
    }

    #[bench]
    #[cfg(not(debug_assertions))]
    fn std_inv_sqrt64(bencher: &mut Bencher) {
        let mut x = 0.01f64;

        bencher.iter(move || {
            for _ in 0 .. 10000 {
                black_box(x / x.sqrt());
                x += 0.01;
            }
        });
    }
}
