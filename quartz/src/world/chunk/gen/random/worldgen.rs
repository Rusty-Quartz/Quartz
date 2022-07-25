use crate::world::chunk::gen::random::{
    java::JavaRandom,
    legacy_random::LegacyRandom,
    BitRandomSource,
    RandomSource,
};

pub struct WorldgenRandom<R: RandomSource> {
    rand_source: R,
    java_rand: JavaRandom,
    count: i32,
}

impl<R: RandomSource> WorldgenRandom<R> {
    pub fn new(rand_source: R) -> WorldgenRandom<R> {
        WorldgenRandom {
            rand_source,
            java_rand: JavaRandom::with_seed(0),
            count: 0,
        }
    }

    pub fn next(&mut self, bits: i32) -> i32 {
        self.count += 1;

        self.next_bits(bits)
    }

    pub fn set_decoration_seed(
        &mut self,
        level_seed: i64,
        min_chunk_block_x: i32,
        min_chunk_block_z: i32,
    ) -> i64 {
        self.set_seed(level_seed);
        let l = self.next_long() | 1;
        let m = self.next_long() | 1;
        let n = (min_chunk_block_x as i64 * l as i64 + min_chunk_block_z as i64 * m as i64)
            ^ level_seed;
        self.set_seed(n);
        n
    }

    pub fn set_feature_seed(&mut self, decoration_seed: i64, index: i32, decoration_step: i32) {
        let l = decoration_seed + index as i64 + (10000 * decoration_step) as i64;
        self.set_seed(l);
    }

    pub fn set_large_feature_seed(&mut self, base_seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(base_seed);
        let l = self.next_long();
        let m = self.next_long();
        let n = (chunk_x as i64 * l) ^ (chunk_z as i64 * m) ^ base_seed;
        self.set_seed(n);
    }

    pub fn set_large_feature_with_salt(
        &mut self,
        level_seed: i64,
        region_x: i32,
        region_z: i32,
        salt: i32,
    ) {
        let l = region_x as i64 * 341873128712
            + region_z as i64 * 132897987541
            + level_seed
            + salt as i64;
        self.set_seed(l);
    }

    pub fn seed_slime_chunks(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        level_seed: i64,
        salt: i64,
    ) -> JavaRandom {
        JavaRandom::with_seed(
            (level_seed
                + (chunk_x * chunk_x * 4987142) as i64
                + (chunk_x * 5947611) as i64
                + (chunk_z * chunk_z) as i64 * 4392871
                + (chunk_z * 389711) as i64)
                ^ salt,
        )
    }
}

impl<R: RandomSource> RandomSource for WorldgenRandom<R> {
    type Positional = R::Positional;

    fn fork(&mut self) -> Self {
        self.rand_source.fork();
        todo!();
    }

    fn fork_positional(&mut self) -> Self::Positional {
        self.rand_source.fork_positional()
    }

    fn set_seed(&mut self, seed: i64) {
        self.rand_source.set_seed(seed)
    }

    fn next_int(&mut self) -> i32 {
        self.java_rand.next_int()
    }

    fn next_int_bounded(&mut self, bound: u32) -> i32 {
        self.java_rand.next_int_bounded(bound as i32)
    }

    fn next_long(&mut self) -> i64 {
        self.java_rand.next_long()
    }

    fn next_bool(&mut self) -> bool {
        self.java_rand.next_bool()
    }

    fn next_float(&mut self) -> f32 {
        self.java_rand.next_float()
    }

    fn next_double(&mut self) -> f64 {
        self.java_rand.next_double()
    }
}
