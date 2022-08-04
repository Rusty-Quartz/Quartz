// The numbers provided in the asserts were gotten by
// running the different prngs via a custom minecraft mod
// these tests currently match vanilla 1.18.2 but I don't see
// them changing the prngs too much in the future


#[test]
fn xoroshiro_test() {
    use quartz::world::chunk::gen::random::{
        xoroshiro::XoroshiroRandom,
        PositionalRandomBuilder,
        Random,
    };

    let mut rand = Random::new(XoroshiroRandom::new(12345));
    assert_eq!(rand.next_int(), 57184507);
    assert_eq!(rand.next_int_bounded(64), 52);
    assert!(rand.next_bool());

    // actual evals are given based on my machine
    // matching digits are in parens
    println!("{}", rand.next_double()); // (0.06648869)129932213
    println!("{}", rand.next_float()); // (0.56348157)
    println!("{}", rand.next_gaussian()); // (1.24300)53633158613

    // the floats are decently accurate, but due to them involving a
    // multiplication it will be platform dependent how close they are
    // if we can figure out how to replace the muls with bit ops these will be enabled
    // you can also enable them to make sure they're decently accurate
    // assert_eq!(rand.next_double(), 0.06648869277406588);
    // assert_eq!(rand.next_float(), 0.56348157);
    // assert_eq!(rand.next_gaussian(), 1.24300259892275);
    assert_eq!(rand.next_long(), 6109844536179375000);
    rand.consume(41);
    assert_eq!(rand.next_int_in_range(67, 30), 39);
    let mut rand2 = rand.fork();
    assert_eq!(rand2.next_int(), 670415208);
    let pos_rand = rand2.fork_positional();
    let mut rand3 = Random::new(pos_rand.fork_at(12, 32, 15));
    assert_eq!(rand3.next_int(), -803388550);
    let pos_rand2 = rand3.fork_positional();
    let mut rand4 = Random::new(pos_rand2.fork_from_hashed_string("Quartz"));
    assert_eq!(rand4.next_int(), 692256388);
}

#[test]
fn legacy_random_test() {
    use quartz::world::chunk::gen::random::{
        legacy_random::LegacyRandom,
        PositionalRandomBuilder,
        Random,
    };

    let mut rand = Random::new(LegacyRandom::new(12345));
    assert_eq!(rand.next_int(), 1553932502);
    assert_eq!(rand.next_int_bounded(64), 32);
    assert!(rand.next_bool());

    // actual evals are given based on my machine
    // matching digits are in parens
    println!("{}", rand.next_double()); // (0.917114)6820182604
    println!("{}", rand.next_float()); // (0.037672937)
    println!("{}", rand.next_gaussian()); // (-0.7423811)635316823

    // the floats are decently accurate, but due to them involving a
    // multiplication it will be platform dependent how close they are
    // if we can figure out how to replace the muls with bit ops these will be enabled
    // you can also enable them to make sure they're decently accurate
    // assert_eq!(rand.next_double(), 0.9171147023602031);
    // assert_eq!(rand.next_float(), 0.037672937);
    // assert_eq!(rand.next_gaussian(), -0.7423811735009279);
    assert_eq!(rand.next_long(), 6440041613324510652);
    rand.consume(41);
    assert_eq!(rand.next_int_in_range(67, 30), 66);
    let mut rand2 = rand.fork();
    assert_eq!(rand2.next_int(), 1255902220);
    let pos_rand = rand2.fork_positional();
    let mut rand3 = Random::new(pos_rand.fork_at(12, 32, 15));
    assert_eq!(rand3.next_int(), -1770212012);
    let pos_rand2 = rand3.fork_positional();
    let mut rand4 = Random::new(pos_rand2.fork_from_hashed_string("Quartz"));
    assert_eq!(rand4.next_int(), -116857100);
}

#[test]
fn java_rand_test() {
    use quartz::world::chunk::gen::random::java::JavaRandom;

    let mut rand = JavaRandom::with_seed(12345);
    assert_eq!(rand.next_int(), 1553932502);
    assert_eq!(rand.next_int_bounded(64), 32);
    assert!(rand.next_bool());

    // these do contain a div so there is the chance they're not fully accurate on every machine
    // but they seem to be accurate on my machine so shrug
    assert_eq!(rand.next_double(), 0.9171147023602031);
    assert_eq!(rand.next_float(), 0.037672937);
    assert_eq!(rand.next_gaussian(), -0.7423811735009279);

    assert_eq!(rand.next_long(), 6440041613324510652);
}

#[test]
fn worldgen_random_test() {
    use quartz::world::chunk::gen::random::{
        legacy_random::LegacyRandom,
        worldgen::{seed_slime_chunks, WorldgenRandom},
        PositionalRandomBuilder,
        Random,
    };

    let mut rand = Random::new(WorldgenRandom::new(LegacyRandom::new(12345)));
    assert_eq!(rand.next_int(), 1553932502);
    assert_eq!(rand.next_int_bounded(64), 32);
    assert!(rand.next_bool());

    // these do contain a div so there is the chance they're not fully accurate on every machine
    // but they seem to be accurate on my machine so shrug
    assert_eq!(rand.next_double(), 0.9171147023602031);
    assert_eq!(rand.next_float(), 0.037672937);
    assert_eq!(rand.next_gaussian(), -0.7423811735009279);

    assert_eq!(rand.next_long(), 6440041613324510652);
    rand.consume(41);
    assert_eq!(rand.next_int_in_range(67, 30), 66);
    let mut rand2 = rand.fork();
    assert_eq!(rand2.next_int(), 1255902220);
    let pos_rand = rand2.fork_positional();
    let mut rand3 = Random::new(pos_rand.fork_at(12, 32, 15));
    assert_eq!(rand3.next_int(), -1770212012);
    let pos_rand2 = rand3.fork_positional();
    let mut rand4 = Random::new(pos_rand2.fork_from_hashed_string("Quartz"));
    assert_eq!(rand4.next_int(), -116857100);

    // worldgen specific stuff
    rand.set_decoration_seed(2345, 12, 34);
    assert_eq!(rand.next_int(), 1973117571);
    rand.set_feature_seed(54321, 3, 12);
    assert_eq!(rand.next_int(), -39835360);
    rand.set_large_feature_seed(67890, 1, 3);
    assert_eq!(rand.next_int(), 1957500698);
    rand.set_large_feature_with_salt(2468, 12, 14, 543);
    assert_eq!(rand.next_int(), -1276341889);
    // for vanilla the salt will always be 987234911 so we just use that
    let mut rand5 = seed_slime_chunks(23, 12, 3344, 987234911);
    assert_eq!(rand5.next_int(), 389151295);
}
