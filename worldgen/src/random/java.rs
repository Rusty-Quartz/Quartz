use std::{
    marker::PhantomData,
    sync::atomic::{AtomicI64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static SEED_UNIQUIFIER: AtomicI64 = AtomicI64::new(8682522807148012);
const ADDEND: i64 = 0xB;
const DOUBLE_UNIT: f64 = 1.1102230246251565E-16;
const MASK: i64 = (1 << 48) - 1;
const MULTIPLIER: i64 = 0x5DEECE66D;

/// A PRNG that emulates the Random class in the java standard library
pub struct JavaRandom {
    pub(super) inner: JavaRandomInner<JavaRandom>,
}

impl JavaRandom {
    /// Create a new JavaRandom with a seed based off of the current time
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

    /// Creates a new JavaRandom with the given seed
    ///
    /// The seed that is set is actually scrambled using `(seed ^ 0x5DEECE66D) & ((1 << 48) - 1)`
    pub fn with_seed(seed: i64) -> JavaRandom {
        JavaRandom {
            inner: JavaRandomInner::with_seed(seed),
        }
    }

    /// Creates a new JavaRandom with the given seed
    ///
    /// this differes from [with_seed](JavaRandom::with_seed) because that will preform a scramble operation first
    ///
    /// only use this if you are sure you need a seed to be a specific value
    pub fn with_raw_seed(seed: i64) -> JavaRandom {
        JavaRandom {
            inner: JavaRandomInner::with_raw_seed(seed),
        }
    }

    const fn initial_scrable(seed: i64) -> i64 {
        (seed ^ MULTIPLIER) & MASK
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.inner.seed = JavaRandom::initial_scrable(seed);
        self.inner.have_next_next_gaussian = false;
    }

    pub fn next_bytes(&mut self, bytes: &mut [u8]) {
        self.inner.next_bytes(&mut (), bytes)
    }

    pub fn next_int(&mut self) -> i32 {
        self.inner.next_int(&mut ())
    }

    pub fn next_int_bounded(&mut self, bound: i32) -> i32 {
        self.inner.next_int_bounded(&mut (), bound)
    }

    pub fn next_long(&mut self) -> i64 {
        self.inner.next_long(&mut ())
    }

    pub fn next_bool(&mut self) -> bool {
        self.inner.next_bool(&mut ())
    }

    pub fn next_float(&mut self) -> f32 {
        self.inner.next_float(&mut ())
    }

    pub fn next_double(&mut self) -> f64 {
        self.inner.next_double(&mut ())
    }

    pub fn next_gaussian(&mut self) -> f64 {
        self.inner.next_gaussian(&mut ())
    }
}

impl Default for JavaRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaRandomByteSource for JavaRandom {
    type Source = ();

    fn next(_source: &mut Self::Source, java_random: &mut JavaRandomInner<Self>, bits: i32) -> i32 {
        let old_seed = java_random.seed;
        let next_seed = (old_seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND)) & MASK;
        java_random.seed = next_seed;
        (next_seed >> (48 - bits)) as i32
    }
}

pub trait JavaRandomByteSource
where Self: Sized
{
    type Source;

    fn next(source: &mut Self::Source, java_random: &mut JavaRandomInner<Self>, bits: i32) -> i32;
}

pub struct JavaRandomInner<BS: JavaRandomByteSource> {
    seed: i64,
    have_next_next_gaussian: bool,
    next_next_gaussian: f64,
    __source: PhantomData<BS>,
}

impl<BS: JavaRandomByteSource> JavaRandomInner<BS> {
    /// Creates a new JavaRandomInner with the given seed
    ///
    /// The seed that is set is actually scrambled using `(seed ^ 0x5DEECE66D) & ((1 << 48) - 1)`
    pub fn with_seed(seed: i64) -> JavaRandomInner<BS> {
        JavaRandomInner {
            seed: JavaRandom::initial_scrable(seed),
            have_next_next_gaussian: false,
            next_next_gaussian: 0.0,
            __source: PhantomData,
        }
    }

    /// Creates a new JavaRandomInner with the given seed
    ///
    /// this differes from [with_seed](JavaRandom::with_seed) because that will preform a scramble operation first
    ///
    /// only use this if you are sure you need a seed to be a specific value
    pub fn with_raw_seed(seed: i64) -> JavaRandomInner<BS> {
        JavaRandomInner {
            seed,
            have_next_next_gaussian: false,
            next_next_gaussian: 0.0,
            __source: PhantomData,
        }
    }

    pub fn next_bytes(&mut self, source: &mut BS::Source, bytes: &mut [u8]) {
        for i in 0 .. bytes.len() {
            let mut rnd = self.next_int(source);
            let mut n = usize::min(bytes.len() - i, 4) as i32;

            while n > 0 {
                n -= 1;

                bytes[i] = rnd as u8;

                rnd >>= 8;
            }
        }
    }

    pub fn next_int(&mut self, source: &mut BS::Source) -> i32 {
        BS::next(source, self, 32)
    }

    pub fn next_int_bounded(&mut self, source: &mut BS::Source, bound: i32) -> i32 {
        if bound <= 0 {
            panic!("JavaRandom next_int_bounded bound needs to be greator than 0")
        }
        let mut bits = BS::next(source, self, 31);

        if (bound & (bound - 1)) == 0 {
            ((bound as i64).wrapping_mul(bits as i64) >> 31) as i32
        } else {
            let mut val = bits % bound;

            while bits - val + (bound - 1) < 0 {
                bits = BS::next(source, self, 31);
                val = bits % bound;
            }

            val
        }
    }

    pub fn next_long(&mut self, source: &mut BS::Source) -> i64 {
        ((BS::next(source, self, 32) as i64) << 32) + BS::next(source, self, 32) as i64
    }

    pub fn next_bool(&mut self, source: &mut BS::Source) -> bool {
        BS::next(source, self, 1) != 0
    }

    pub fn next_float(&mut self, source: &mut BS::Source) -> f32 {
        BS::next(source, self, 24) as f32 / (1 << 24) as f32
    }

    pub fn next_double(&mut self, source: &mut BS::Source) -> f64 {
        (((BS::next(source, self, 26) as i64) << 27) + BS::next(source, self, 27) as i64) as f64
            / DOUBLE_UNIT
    }

    pub fn next_gaussian(&mut self, source: &mut BS::Source) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            self.next_next_gaussian
        } else {
            let mut v1 = 2.0 * self.next_double(source) - 1.0;
            let mut v2 = 2.0 * self.next_double(source) - 1.0;
            let mut s = v1 * v1 + v2 * v2;

            while s >= 1.0 || s == 0.0 {
                v1 = 2.0 * self.next_double(source) - 1.0;
                v2 = 2.0 * self.next_double(source) - 1.0;
                s = v1 * v1 + v2 * v2;
            }

            let multiplier = f64::sqrt(-2.0 * f64::ln(s) / s);
            self.next_next_gaussian = v2 * multiplier;
            self.have_next_next_gaussian = true;
            v1 * multiplier
        }
    }
}
