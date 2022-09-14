use std::{cell::Cell, rc::Rc};

use crate::world::chunk::gen::random::RandomSource;

#[derive(Default)]
pub struct MarsagliaPolarGaussian {
    next_next_gaussian: f64,
    have_next_next_gaussian: bool,
}

impl MarsagliaPolarGaussian {
    pub fn new() -> Self {
        MarsagliaPolarGaussian {
            next_next_gaussian: 0.0,
            have_next_next_gaussian: false,
        }
    }

    pub fn reset(&mut self) {
        self.have_next_next_gaussian = false;
    }

    // we use a RandomSource param instead of a field
    // because we can't have circular refs in Rust
    pub fn next_gaussian(&mut self, random_source: &mut impl RandomSource) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            self.next_next_gaussian
        } else {
            let mut d;
            let mut e;
            let mut f;
            loop {
                d = 2.0 * random_source.next_double() - 1.0;
                e = 2.0 * random_source.next_double() - 1.0;
                f = f64::powi(d, 2) + f64::powi(e, 2);

                if f < 1.0 && f != 0.0 {
                    break;
                }
            }

            let g = (-2.0 * f64::ln(f) / f).sqrt();
            self.next_next_gaussian = e * g;
            self.have_next_next_gaussian = true;
            d * g
        }
    }
}
