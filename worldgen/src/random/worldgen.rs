use crate::random::{
    java::{JavaRandom, JavaRandomByteSource, JavaRandomInner},
    marsaglia_polar::MarsagliaPolarGaussian,
    Random,
    RandomSource,
};

// this is attached to WorldgenRandom in vanilla
// but since that would require us specifying a generic everytime we use it
// we just have it be a free function
/// Creates a [JavaRandom] for determining if a chunk should be a slime chunk
///
/// In vanilla `salt` is always 987234911
pub fn seed_slime_chunks(chunk_x: i32, chunk_z: i32, level_seed: i64, salt: i64) -> JavaRandom {
    JavaRandom::with_seed(
        level_seed
            .wrapping_add(chunk_x.wrapping_mul(chunk_x).wrapping_mul(4987142) as i64)
            .wrapping_add(chunk_x.wrapping_mul(5947611) as i64)
            .wrapping_add(
                ((chunk_z.wrapping_mul(chunk_z) as i64).wrapping_mul(4392871))
                    .wrapping_add(chunk_z.wrapping_mul(389711) as i64),
            )
            ^ salt,
    )
}

pub struct WorldgenRandom<R: RandomSource> {
    rand_source: R,
    java_rand: JavaRandomInner<WorldgenRandom<R>>,
    count: i32,
}

impl<R: RandomSource> WorldgenRandom<R> {
    pub fn new(rand_source: R) -> WorldgenRandom<R> {
        WorldgenRandom {
            rand_source,
            java_rand: JavaRandomInner::with_raw_seed(0),
            count: 0,
        }
    }

    pub fn set_decoration_seed(
        &mut self,
        level_seed: i64,
        min_chunk_block_x: i32,
        min_chunk_block_z: i32,
        gaussian: &mut MarsagliaPolarGaussian,
    ) -> i64 {
        self.set_seed(level_seed, gaussian);
        let l = self.next_long() | 1;
        let m = self.next_long() | 1;
        let n = (min_chunk_block_x as i64 * l + min_chunk_block_z as i64 * m) ^ level_seed;
        self.set_seed(n, gaussian);
        n
    }

    pub fn set_feature_seed(
        &mut self,
        decoration_seed: i64,
        index: i32,
        decoration_step: i32,
        gaussian: &mut MarsagliaPolarGaussian,
    ) {
        let l = decoration_seed + index as i64 + (10000 * decoration_step) as i64;
        self.set_seed(l, gaussian);
    }

    pub fn set_large_feature_seed(
        &mut self,
        base_seed: i64,
        chunk_x: i32,
        chunk_z: i32,
        gaussian: &mut MarsagliaPolarGaussian,
    ) {
        self.set_seed(base_seed, gaussian);
        let l = self.next_long();
        let m = self.next_long();
        let n = (chunk_x as i64 * l) ^ (chunk_z as i64 * m) ^ base_seed;
        self.set_seed(n, gaussian);
    }

    pub fn set_large_feature_with_salt(
        &mut self,
        level_seed: i64,
        region_x: i32,
        region_z: i32,
        salt: i32,
        gaussian: &mut MarsagliaPolarGaussian,
    ) {
        let l = region_x as i64 * 341873128712
            + region_z as i64 * 132897987541
            + level_seed
            + salt as i64;
        self.set_seed(l, gaussian);
    }
}

impl<R: RandomSource> RandomSource for WorldgenRandom<R> {
    type Positional = R::Positional;

    fn fork(&mut self) -> Self {
        let rand = self.rand_source.fork();
        WorldgenRandom::new(rand)
    }

    fn fork_positional(&mut self) -> Self::Positional {
        self.rand_source.fork_positional()
    }

    fn set_seed(&mut self, seed: i64, gaussian: &mut MarsagliaPolarGaussian) {
        self.rand_source.set_seed(seed, gaussian)
    }

    fn next_int(&mut self) -> i32 {
        self.count += 1;
        self.next_bits(32)
    }

    fn next_bits(&mut self, bits: i32) -> i32 {
        self.count += 1;

        self.rand_source.next_bits(bits)
    }

    fn next_int_bounded(&mut self, bound: u32) -> i32 {
        self.java_rand
            .next_int_bounded(&mut self.rand_source, bound as i32)
    }

    fn next_long(&mut self) -> i64 {
        self.java_rand.next_long(&mut self.rand_source)
    }

    fn next_bool(&mut self) -> bool {
        self.java_rand.next_bool(&mut self.rand_source)
    }

    fn next_float(&mut self) -> f32 {
        self.java_rand.next_float(&mut self.rand_source)
    }

    fn next_double(&mut self) -> f64 {
        self.java_rand.next_double(&mut self.rand_source)
    }
}

impl<R: RandomSource> Random<WorldgenRandom<R>> {
    pub fn set_decoration_seed(
        &mut self,
        level_seed: i64,
        min_chunk_block_x: i32,
        min_chunk_block_z: i32,
    ) -> i64 {
        self.source.set_seed(level_seed, &mut self.gaussian);
        let l = self.next_long() | 1;
        let m = self.next_long() | 1;
        let n = (min_chunk_block_x as i64)
            .wrapping_mul(l)
            .wrapping_add((min_chunk_block_z as i64).wrapping_mul(m))
            ^ level_seed;
        self.source.set_seed(n, &mut self.gaussian);
        n
    }

    pub fn set_feature_seed(&mut self, decoration_seed: i64, index: i32, decoration_step: i32) {
        let l = decoration_seed
            .wrapping_add(index as i64)
            .wrapping_add(decoration_step.wrapping_mul(10000) as i64);
        self.source.set_seed(l, &mut self.gaussian);
    }

    pub fn set_large_feature_seed(&mut self, base_seed: i64, chunk_x: i32, chunk_z: i32) {
        self.source.set_seed(base_seed, &mut self.gaussian);
        let l = self.next_long();
        let m = self.next_long();
        let n = (chunk_x as i64).wrapping_mul(l) ^ (chunk_z as i64).wrapping_mul(m) ^ base_seed;
        self.source.set_seed(n, &mut self.gaussian);
    }

    pub fn set_large_feature_with_salt(
        &mut self,
        level_seed: i64,
        region_x: i32,
        region_z: i32,
        salt: i32,
    ) {
        let l = (region_x as i64)
            .wrapping_mul(341873128712)
            .wrapping_add((region_z as i64).wrapping_mul(132897987541))
            .wrapping_add(level_seed)
            .wrapping_add(salt as i64);
        self.source.set_seed(l, &mut self.gaussian);
    }
}

impl<R: RandomSource> JavaRandomByteSource for WorldgenRandom<R> {
    type Source = R;

    fn next(source: &mut Self::Source, _java_random: &mut JavaRandomInner<Self>, bits: i32) -> i32 {
        source.next_bits(bits)
    }
}
