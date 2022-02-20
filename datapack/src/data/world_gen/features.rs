use qdat::{world::location::BlockPosition, UnlocalizedName};
use serde::{Deserialize, Serialize};

use crate::data::datatypes::{BlockPredicate, ValueOrList};

use super::{
    carvers::{FloatProvider, HeightProvider},
    noise_settings::BlockState,
    processors::ProcessorPredicate,
};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum Feature {
    #[serde(rename = "minecraft:bamboo")]
    Bamboo { probability: f32 },
    #[serde(rename = "minecraft:basalt_columns")]
    BasaltColumns {
        reach: IntProvider,
        height: IntProvider,
    },
    #[serde(rename = "minecraft:basalt_pillar")]
    BasaltPillar {},
    #[serde(rename = "minecraft:block_column")]
    BlockColumn {
        direction: Direction,
        allowed_placement: BlockPredicate,
        prioritize_tip: bool,
        layers: Vec<BlockColumnLayer>,
    },
    #[serde(rename = "minecraft:block_pile")]
    BlockPile { state_provider: BlockStateProvider },
    #[serde(rename = "minecraft:blue_ice")]
    BlueIce {},
    #[serde(rename = "minecraft:bonus_chest")]
    BonusChest {},
    #[serde(rename = "minecraft:chorus_plant")]
    ChorusPlant {},
    #[serde(rename = "minecraft:coral_claw")]
    CoralClaw {},
    #[serde(rename = "minecraft:coral_mushroom")]
    CoralMushroom {},
    #[serde(rename = "minecraft:coral_tree")]
    CoralTree {},
    // #[serde(rename = "minecraft:decorated")]
    // Decorated {
    //     decorator: Decorators,
    //     feature: String,
    // },
    #[serde(rename = "minecraft:delta_feature")]
    DeltaFeature {
        contents: BlockState,
        rim: BlockState,
        size: IntProvider,
        rim_size: IntProvider,
    },
    #[serde(rename = "minecraft:desert_well")]
    DesertWell {},
    #[serde(rename = "minecraft:disk")]
    Disk {
        state: BlockState,
        radius: IntProvider,
        half_height: i32,
        targets: Vec<BlockState>,
    },
    #[serde(rename = "minecraft:dripstone_cluster")]
    DripstoneCluster {
        floor_to_ceiling_search_range: i32,
        height: IntProvider,
        radius: IntProvider,
        max_stalagmite_stalactite_height_diff: i32,
        height_deviation: i32,
        dripstone_block_layer_thickness: IntProvider,
        density: FloatProvider,
        wetness: FloatProvider,
        chance_of_dripstone_column_at_max_distance_from_center: f32,
        max_distance_from_edge_affecting_chance_of_dripstone_column: i32,
        max_distance_from_center_affecting_height_bias: i32,
    },
    #[serde(rename = "minecraft:end_gateway")]
    EndGateway {
        exact: bool,
        // Technically this should have y be an i32 but I don't care
        exit: Option<BlockPosition>,
    },
    #[serde(rename = "minecraft:end_island")]
    EndIsland {},
    #[serde(rename = "minecraft:end_spike")]
    EndSpike {
        #[serde(default = "Default::default")]
        crystal_invulnerable: bool,
        cyrstal_beam_target: Option<BlockPosition>,
        spikes: Vec<EndSpikeConfg>,
    },
    #[serde(rename = "minecraft:fill_layer")]
    FillLayer { state: BlockState, height: i32 },
    #[serde(rename = "minecraft:flower")]
    Flower {},
    #[serde(rename = "minecraft:forest_rock")]
    ForestRock { state: BlockState },
    #[serde(rename = "minecraft:fossil")]
    Fossil {
        fossil_structures: Vec<UnlocalizedName>,
        overlay_structures: Vec<UnlocalizedName>,
        fossil_processors: UnlocalizedName,
        overlay_processors: UnlocalizedName,
        max_empty_corners_allowed: i32,
    },
    #[serde(rename = "minecraft:freeze_top_layer")]
    FreezeTopLayer {},
    #[serde(rename = "minecraft:geode")]
    Geode {
        blocks: Box<GeodeBlocks>,
        layers: GeodeLayers,
        crack: GeodeCrack,
        noise_multiplier: Option<f64>,
        use_potential_placements_chance: Option<f64>,
        use_alternate_layer0_chance: Option<f64>,
        placements_require_layer0_alternate: Option<bool>,
        outer_wall_distance: Option<IntProvider>,
        distribution_points: Option<IntProvider>,
        point_offset: Option<IntProvider>,
        min_gen_offset: Option<i32>,
        max_gen_offset: Option<i32>,
    },
    #[serde(rename = "minecraft:glow_lichen")]
    GlowLichen {
        search_range: Option<i32>,
        chance_of_spreading: Option<f32>,
        #[serde(default = "Default::default")]
        can_place_on_floor: bool,
        #[serde(default = "Default::default")]
        can_place_on_ceiling: bool,
        #[serde(default = "Default::default")]
        can_place_on_wall: bool,
        can_be_placed_on: Vec<UnlocalizedName>,
    },
    #[serde(rename = "minecraft:growing_plant")]
    GrowingPlant {
        direction: Direction,
        allow_water: bool,
        height_distribution: Vec<GrowingPlantHeight>,
        body_provider: BlockStateProvider,
        head_provider: BlockStateProvider,
    },
    #[serde(rename = "minecraft:glowstone_blob")]
    GlowstoneBlob {},
    #[serde(rename = "minecraft:huge_brown_mushroom")]
    HugeBrownMushroom {
        cap_provider: BlockStateProvider,
        stem_provider: BlockStateProvider,
        fliage_radius: Option<i32>,
    },
    #[serde(rename = "minecraft:huge_fungus")]
    HugeFungus {
        hat_state: BlockState,
        decor_state: BlockState,
        stem_state: BlockState,
        valid_base_block: BlockState,
        #[serde(default = "Default::default")]
        planted: bool,
    },
    #[serde(rename = "minecraft:huge_red_mushroom")]
    HugeRedMushroom {
        cap_provider: BlockStateProvider,
        stem_provider: BlockStateProvider,
        fliage_radius: Option<i32>,
    },
    #[serde(rename = "minecraft:ice_patch")]
    IcePatch {},
    #[serde(rename = "minecraft:ice_spike")]
    IceSpike {},
    #[serde(rename = "minecraft:iceberg")]
    Iceberg { state: BlockState },
    #[serde(rename = "minecraft:kelp")]
    Kelp {},
    #[serde(rename = "minecraft:lake")]
    Lake {
        fluid: BlockStateProvider,
        barrier: BlockStateProvider,
    },
    #[serde(rename = "minecraft:large_dripstone")]
    LargeDripstone {
        floor_to_ceiling_search_range: Option<i32>,
        column_radius: IntProvider,
        height_scale: FloatProvider,
        max_column_radius_to_cave_height_ratio: f32,
        stalactite_bluntness: FloatProvider,
        stalagmite_bluntness: FloatProvider,
        wind_speed: FloatProvider,
        min_radius_for_wind: i32,
        min_bluntness_for_wind: f32,
    },
    #[serde(rename = "minecraft:monster_room")]
    MonsterRoom {},
    #[serde(rename = "minecraft:nether_forest_vegetation")]
    NetherForestVegetation { state_provider: BlockStateProvider },
    #[serde(rename = "minecraft:netherrack_replace_blobs")]
    NetherrackReplaceBlobs {
        state: BlockState,
        target: BlockState,
        radius: IntProvider,
    },
    #[serde(rename = "minecraft:no_bonemeal_flower")]
    NoBonemealFlower {
        y_spread: Option<i32>,
        xz_spread: Option<i32>,
        tries: Option<i32>,
        feature: PlacedFeature,
    },
    #[serde(rename = "minecraft:no_op")]
    NoOp {},
    #[serde(rename = "minecraft:ore")]
    Ore {
        size: i32,
        discard_chance_on_air_exposure: f32,
        targets: Vec<BlockStateTarget>,
    },
    #[serde(rename = "minecraft:pointed_dripstone")]
    PointedDripstone {
        chance_of_taller_dripstone: Option<f32>,
        chance_of_directional_spread: Option<f32>,
        chance_of_spread_radius2: Option<f32>,
        chance_of_spread_radius3: Option<f32>,
    },
    #[serde(rename = "minecraft:random_boolean_selector")]
    RandomBooleanSelector {
        feature_false: PlacedFeature,
        feature_true: PlacedFeature,
    },
    #[serde(rename = "minecraft:random_patch")]
    RandomPatch {
        y_spread: Option<i32>,
        xz_spread: Option<i32>,
        tries: Option<i32>,
        feature: PlacedFeature,
    },
    #[serde(rename = "minecraft:random_selector")]
    RandomSelector {
        features: Vec<RandomFeature>,
        default: PlacedFeature,
    },
    #[serde(rename = "minecraft:replace_single_block")]
    ReplaceSingleBlock { targets: Vec<BlockStateTarget> },
    #[serde(rename = "minecraft:root_system")]
    RootSystem {
        required_vertical_space_for_tree: i32,
        root_radius: i32,
        root_placement_attempts: i32,
        root_column_max_height: i32,
        hanging_root_radius: i32,
        hanging_roots_vertical_span: i32,
        hanging_root_placement_attempts: i32,
        allowed_vertical_water_for_tree: i32,
        root_replaceable: UnlocalizedName,
        root_state_provider: BlockStateProvider,
        hanging_root_state_provider: BlockStateProvider,
        feature: PlacedFeature,
    },
    #[serde(rename = "minecraft:scattered_ore")]
    ScatteredOre {
        size: i32,
        discard_chance_on_air_exposure: f32,
        targets: Vec<BlockStateTarget>,
    },
    #[serde(rename = "minecraft:sea_pickle")]
    SeaPickle { count: IntProvider },
    #[serde(rename = "minecraft:seagrass")]
    Seagrass { probability: f32 },
    #[serde(rename = "minecraft:simple_block")]
    SimpleBlock { to_place: BlockStateProvider },
    #[serde(rename = "minecraft:simple_random_selector")]
    SimpleRandomSelector { features: Vec<PlacedFeature> },
    #[serde(rename = "minecraft:small_dripstone")]
    SmallDripstone {
        max_placements: Option<i32>,
        empty_space_search_radius: Option<i32>,
        max_offset_from_origin: Option<i32>,
        chance_of_taller_dripstone: Option<f32>,
    },
    #[serde(rename = "minecraft:spring_feature")]
    SpringFeature {
        state: BlockState,
        rock_count: Option<i32>,
        hole_count: Option<i32>,
        requires_block_below: Option<bool>,
        valid_blocks: ValueOrList<UnlocalizedName>,
    },
    #[serde(rename = "minecraft:tree")]
    Tree {
        #[serde(default = "Default::default")]
        ignore_vines: bool,
        #[serde(default = "Default::default")]
        force_dirt: bool,
        minimum_size: TreeMinimumSize,
        dirt_provider: BlockStateProvider,
        trunk_provider: BlockStateProvider,
        foliage_provider: BlockStateProvider,
        trunk_placer: TreeTrunkPlacer,
        foliage_placer: TreeFoliagePlacer,
        decorators: Vec<TreeDecorator>,
    },
    #[serde(rename = "minecraft:twisting_vines")]
    TwistingVines {},
    #[serde(rename = "minecraft:underwater_magma")]
    UnderwaterMagma {},
    #[serde(rename = "minecraft:vegetation_patch")]
    VegetationPatch {
        surface: SurfaceType,
        depth: IntProvider,
        vertical_range: i32,
        extra_bottom_block_chance: f32,
        extra_edge_column_chance: f32,
        vegetation_chance: f32,
        xz_radius: IntProvider,
        replaceable: String,
        ground_state: BlockStateProvider,
        vegetation_feature: PlacedFeature,
    },
    #[serde(rename = "minecraft:vines")]
    Vines {},
    #[serde(rename = "minecraft:void_start_platform")]
    VoidStartPlatform {},
    #[serde(rename = "minecraft:waterlogged_vegetation_patch")]
    WaterloggedVegetationPatch {
        vegetation_chance: f32,
        xz_radius: IntProvider,
        extra_edge_column_chance: f32,
        extra_bottom_block_chance: f32,
        vertical_range: i32,
        vegetation_feature: PlacedFeature,
        surface: SurfaceType,
        depth: IntProvider,
        replaceable: String,
        ground_state: BlockStateProvider,
    },
    #[serde(rename = "minecraft:weeping_vines")]
    WeepingVines {},
}

#[derive(Serialize, Deserialize)]
pub struct BlockColumnLayer {
    height: IntProvider,
    provider: BlockStateProvider,
}


#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlacedFeature {
    Feature(UnlocalizedName),
    ModifiedFeature(ModifiedFeature),
}

#[derive(Serialize, Deserialize)]
pub struct ModifiedFeature {
    feature: ModifiedFeatureEntry,
    placement: Vec<PlacementModifier>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModifiedFeatureEntry {
    Uln(UnlocalizedName),
    Feature(Box<Feature>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlacementModifier {
    #[serde(rename = "minecraft:block_predicate_filter")]
    BlockPredicateFilter { predicate: BlockPredicate },
    #[serde(rename = "minecraft:carving_mask")]
    CarvingMask { step: CarvingStep },
    #[serde(rename = "minecraft:count")]
    Count { count: IntProvider },
    #[serde(rename = "minecraft:count_on_every_layer")]
    CountOnEveryLayer { count: IntProvider },
    #[serde(rename = "minecraft:environment_scan")]
    EnvironmentScan {
        direction_of_search: SearchDirection,
        max_steps: u8,
        target_condition: BlockPredicate,
        allowed_search_condition: Option<BlockPredicate>,
    },
    #[serde(rename = "minecraft:height_range")]
    HeightRange { height: HeightProvider },
    #[serde(rename = "minecraft:heightmap")]
    Heightmap { heightmap: HeightMaps },
    #[serde(rename = "minecraft:in_square")]
    InSquare,
    #[serde(rename = "minecraft:noise_based_count")]
    NoiseBasedCount {
        noise_factor: f64,
        noise_offset: f64,
        noise_to_count_ratio: i32,
    },
    #[serde(rename = "minecraft:noise_threshold_count")]
    NoiseThresholdCount {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    #[serde(rename = "minecraft:random_offset")]
    RandomOffset {
        xz_spread: IntProvider,
        y_spread: IntProvider,
    },
    #[serde(rename = "minecraft:rarity_filter")]
    RarityFilter { chance: i32 },
    #[serde(rename = "minecraft:surface_relative_threshold_filter")]
    SurfaceRelativeThresholdFilter {
        heightmap: HeightMaps,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:surface_water_depth_filter")]
    SurfaceWaterDepthFilter { max_water_depth: i32 },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchDirection {
    Up,
    Down,
}

#[derive(Serialize, Deserialize)]
pub enum Decorators {
    CarvingMask {
        step: CarvingStep,
    },
    CaveSurface {
        surface: CaveSurface,
        floor_to_ceiling_search_range: i32,
    },
    Chance {
        chance: i32,
    },
    Count {
        count: IntProvider,
    },
    CountExtra {
        count: i32,
        extra_count: i32,
        extra_chance: f32,
    },
    CountMultilayer {
        count: IntProvider,
    },
    CountNoise {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    CountNoiseBiased {
        noise_factor: f64,
        #[serde(default = "Default::default")]
        noise_offset: f64,
        noise_to_count_ratio: i32,
    },
    DarkOakTree,
    Decorated {
        outer: Box<Decorators>,
        inner: Box<Decorators>,
    },
    EndGateway,
    Heightmap {
        heightmap: HeightMaps,
    },
    Iceberg,
    LavaLake {
        count: i32,
    },
    Nope,
    Range {
        height: HeightProvider,
    },
    Spraed32Above,
    Square,
    WaterDepthThreshold {
        max_water_depth: i32,
    },
}

#[derive(Serialize, Deserialize)]
pub struct TreeMinimumSize {
    #[serde(flatten)]
    kind: TreeSizeType,
    min_clipped_height: Option<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TreeSizeType {
    #[serde(rename = "minecraft:two_layers_feature_size")]
    TwoLayersFeatureSize {
        limit: Option<i32>,
        lower_size: Option<i32>,
        upper_size: Option<i32>,
    },
    #[serde(rename = "minecraft:three_layers_feature_size")]
    ThreeLayersFeatureSize {
        limit: Option<i32>,
        upper_limit: Option<i32>,
        lower_size: Option<i32>,
        middle_size: Option<i32>,
        upper_size: Option<i32>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub enum TreeFoliagePlacer {
    #[serde(rename = "minecraft:blob_foliage_placer")]
    BlobFoliagePlacer {
        height: i32,
        radius: i32,
        offset: i32,
    },
    #[serde(rename = "minecraft:spruce_foliage_placer")]
    SpruceFoliagePlacer { trunk_height: IntProvider },
    #[serde(rename = "minecraft:pine_foliage_placer")]
    PineFoliagePlacer { height: IntProvider },
    #[serde(rename = "minecraft:acacia_foliage_placer")]
    AcaciaFoliagePlacer,
    #[serde(rename = "minecraft:bush_foliage_placer")]
    BushFoliagePlacer {
        height: i32,
        radius: i32,
        offset: i32,
    },
    #[serde(rename = "minecraft:fancy_foliage_placer")]
    FancyFoliagePlacer {
        height: i32,
        radius: i32,
        offset: i32,
    },
    #[serde(rename = "minecraft:jungle_foliage_placer")]
    JungleFoliagePlacer {
        height: i32,
        radius: i32,
        offset: i32,
    },
    #[serde(rename = "minecraft:mega_pine_foliage_placer")]
    MegaPineFoliagePlacer { crown_height: IntProvider },
    #[serde(rename = "minecraft:dark_oak_foliage_placer")]
    DarkOakFoliagePlacer,
    #[serde(rename = "minecraft:random_spread_foliage_placer")]
    RandomSpreadFoliagePlacer {
        foliage_height: IntProvider,
        leaf_placement_attempts: i32,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TreeDecorator {
    #[serde(rename = "minecraft:trunk_vine")]
    TrunkVine,
    #[serde(rename = "minecraft:leave_vine")]
    LeaveVine,
    #[serde(rename = "minecraft:cocoa")]
    Cocoa { probability: f32 },
    #[serde(rename = "minecraft:beehive")]
    Beehive { probability: f32 },
    #[serde(rename = "minecraft:alter_ground")]
    AlterGround { provider: BlockStateProvider },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub enum TreeTrunkPlacer {
    #[serde(rename = "minecraft:straight_trunk_placer")]
    StraightTrunkPlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
    },
    #[serde(rename = "minecraft:forking_trunk_placer")]
    ForkingTrunkPlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
    },
    #[serde(rename = "minecraft:giant_trunk_placer")]
    GiantTrunkPlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
    },
    #[serde(rename = "minecraft:mega_jungle_trunk_placer")]
    MegaJunglePlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
    },
    #[serde(rename = "minecraft:dark_oak_trunk_placer")]
    DarkOakTrunkPlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
    },
    #[serde(rename = "minecraft:fancy_trunk_placer")]
    FancyTrunkPlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
    },
    #[serde(rename = "minecraft:bending_trunk_placer")]
    BendingTrunkPlacer {
        #[serde(flatten)]
        settings: TreeTrunkSettings,
        bend_length: IntProvider,
        min_height_for_leaves: Option<i32>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct TreeTrunkSettings {
    base_height: i8,
    height_rand_a: i8,
    height_rand_b: i8,
}

#[derive(Serialize, Deserialize)]
pub struct BlockStateTarget {
    target: ProcessorPredicate,
    state: BlockState,
}

#[derive(Serialize, Deserialize)]
pub struct GeodeBlocks {
    filling_provider: BlockStateProvider,
    inner_layer_provider: BlockStateProvider,
    alternate_inner_layer_provider: BlockStateProvider,
    middle_layer_provider: BlockStateProvider,
    outer_layer_provider: BlockStateProvider,
    inner_placements: Vec<BlockState>,
    cannot_replace: String,
    invalid_blocks: String,
}

#[derive(Serialize, Deserialize)]
pub struct GeodeLayers {
    filling: Option<f64>,
    inner_layer: Option<f64>,
    middle_layer: Option<f64>,
    outer_layer: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct GeodeCrack {
    generate_crack_chance: Option<f64>,
    base_crack_size: Option<f64>,
    crack_point_offset: Option<i32>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HeightMaps {
    MotionBlocking,
    MotionBlockingNoLeaves,
    OceanFloor,
    OceanFloorWg,
    WorldSurface,
    WorldSurfaceWg,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaveSurface {
    Floor,
    Ceiling,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarvingStep {
    Air,
    Liquid,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceType {
    Floor,
    Ceiling,
}

#[derive(Serialize, Deserialize)]
pub struct RandomFeature {
    feature: PlacedFeature,
    chance: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub enum RandomPatchBlockPlacer {
    #[serde(rename = "minecraft:simple_block_placer")]
    SimpleBlockPlacer,
    #[serde(rename = "minecraft:double_block_placer")]
    DoubleBlockPlacer,
    #[serde(rename = "minecraft:column_placer")]
    ColumnPlacer { min_size: i32, extra_size: i32 },
}

#[derive(Serialize, Deserialize)]
pub struct ReplaceBlobsRadius {
    base: i32,
    spread: i32,
}

#[derive(Serialize, Deserialize)]
pub struct GrowingPlantHeight {
    weight: i32,
    data: IntProvider,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Up,
    Down,
    North,
    East,
    South,
    West,
}

#[derive(Serialize, Deserialize)]
pub struct EndSpikeConfg {
    #[serde(rename = "centerX")]
    center_x: Option<i32>,
    #[serde(rename = "centerY")]
    center_y: Option<i32>,
    radius: Option<i32>,
    height: Option<i32>,
    guarded: Option<bool>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntProvider {
    Constant(i32),
    WeightedList {
        // Needed so that we consume all input
        // This should always be "minecraft:weighted_list"
        r#type: String,
        distribution: Vec<WeightedListEntry>,
    },
    Provider(IntProviderInternal),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum IntProviderInternal {
    #[serde(rename = "minecraft:constant")]
    Constant(i32),
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:biased_to_bottom")]
    BiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:clamped")]
    Clamped {
        min_inclusive: i32,
        max_inclusive: i32,
        source: Box<IntProvider>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct WeightedListEntry {
    data: IntProvider,
    weight: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::enum_variant_names)]
pub enum BlockStateProvider {
    #[serde(rename = "minecraft:simple_state_provider")]
    SimpleStateProvider { state: BlockState },
    #[serde(rename = "minecraft:rotated_block_provider")]
    RotatedBlockProvider { state: BlockState },
    #[serde(rename = "minecraft:weighted_state_provider")]
    WeightedStateProvider { entries: Vec<WeightedState> },
    #[serde(rename = "minecraft:plain_flower_provider")]
    PlainFlowerProvider,
    #[serde(rename = "minecraft:forest_flower_provider")]
    ForestFlowerProvider,
    #[serde(rename = "minecraft:randomized_int_state_provider")]
    RandomizedIntStateProvider {
        source: Box<BlockStateProvider>,
        property: String,
        values: IntProvider,
    },
}

#[derive(Serialize, Deserialize)]
pub struct WeightedState {
    data: BlockState,
    weight: i32,
}
