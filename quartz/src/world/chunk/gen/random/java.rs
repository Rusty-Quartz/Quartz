use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SEED_UNIQUIFIER: AtomicI64 = AtomicI64::new(8682522807148012);
const ADDEND: i64 = 0xB;
const DOUBLE_UNIT: f64 = 1.1102230246251565E-16;
const MASK: i64 = (1 << 48) - 1;
const MULTIPLIER: i64 = 0x5DEECE66D;

pub struct JavaRandom {
    seed: i64,
    have_next_next_gaussian: bool,
    next_next_gaussian: f64,
}

impl JavaRandom {
    pub fn new() -> JavaRandom {
        JavaRandom::with_seed(
            JavaRandom::seed_uniquifier()
                ^ SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("UNIX_EPOCH shouldn't ever be after the system time")
                    .as_nanos() as i64,
        )
    }

    // TODO: no idea how orderings work, look at later
    fn seed_uniquifier() -> i64 {
        loop {
            let current = SEED_UNIQUIFIER.load(Ordering::SeqCst);
            let next = current * 1181783497276652981;
            if SEED_UNIQUIFIER
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return next;
            }
        }
    }

    pub fn with_seed(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: JavaRandom::initial_scrable(seed),
            have_next_next_gaussian: false,
            next_next_gaussian: 0.0,
        }
    }

    const fn initial_scrable(seed: i64) -> i64 {
        (seed ^ MULTIPLIER) & MASK
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = JavaRandom::initial_scrable(seed);
        self.have_next_next_gaussian = false;
    }

    fn next(&mut self, bits: i32) -> i32 {
        let old_seed = self.seed;
        let next_seed = (old_seed * MULTIPLIER + ADDEND) & MASK;
        self.seed = next_seed;
        (next_seed >> (48 - bits)) as i32
    }

    pub fn next_bytes(&mut self, bytes: &mut [u8]) {
        for i in 0 .. bytes.len() {
            let mut rnd = self.next_int();
            let mut n = usize::min(bytes.len() - i, 4) as i32;

            while n > 0 {
                n -= 1;

                bytes[i] = rnd as u8;

                rnd >>= 8;
            }
        }
    }

    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    pub fn next_int_bounded(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            panic!("JavaRandom next_int_bounded bound needs to be greator than 0")
        }

        if (bound & -bound) == bound {
            (bound as i64 * ((self.next(31) as i64) >> 31)) as i32
        } else {
            let mut bits = self.next(31);
            let mut val = bits % bound;

            while bits - val + (bound - 1) < 0 {
                bits = self.next(31);
                val = bits % bound;
            }

            val
        }
    }

    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32) + self.next(32) as i64
    }

    pub fn next_bool(&mut self) -> bool {
        self.next(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    pub fn next_double(&mut self) -> f64 {
        (((self.next(26) as i64) << 27) as f64 + self.next(27) as f64) / (1_i64 << 53) as f64
    }

    pub fn next_gaussian(&mut self) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            self.next_next_gaussian
        } else {
            let mut v1 = 2.0 * self.next_double() - 1.0;
            let mut v2 = 2.0 * self.next_double() - 1.0;
            let mut s = v1 * v1 + v2 * v2;

            while s >= 1.0 || s == 0.0 {
                v1 = 2.0 * self.next_double() - 1.0;
                v2 = 2.0 * self.next_double() - 1.0;
                s = v1 * v1 + v2 * v2;
            }

            let multiplier = f64::sqrt(-2.0 * f64::ln(s) / s);
            self.next_next_gaussian = v2 * multiplier;
            self.have_next_next_gaussian = true;
            v1 * multiplier
        }
    }
}
