use crate::{
    network::{PacketBuffer, PacketSerdeError, ReadFromPacket, WriteToPacket},
    world::location::BlockPosition,
};
use quartz_chat::Component;
use quartz_macros::{ReadFromPacket, WriteToPacket};
use quartz_nbt::NbtCompound;
use quartz_util::UnlocalizedName;
use uuid::Uuid;

include!(concat!(env!("OUT_DIR"), "/packet_def_output.rs"));

/// A wraper for a server-bound packet which includes the sender ID.
pub struct WrappedServerBoundPacket {
    /// The ID of the packet sender.
    pub sender: usize,
    /// The packet that was sent.
    pub packet: ServerBoundPacket,
}

impl WrappedServerBoundPacket {
    /// Creates a new wrapper with the given parameters.
    #[inline]
    pub fn new(sender: usize, packet: ServerBoundPacket) -> Self {
        WrappedServerBoundPacket { sender, packet }
    }
}

/// A wraper for client-bound packets used internally for sending packets to the connection thread.
pub enum WrappedClientBoundPacket {
    /// A wrapped packet.
    Packet(ClientBoundPacket),
    /// A raw byte-buffer.
    Buffer(PacketBuffer),
    /// Specifies that the connection should be forcefully terminated.
    Disconnect,
}

#[derive(WriteToPacket)]
pub enum EntityMetadata {
    Byte(i8),
    VarInt(#[packet_serde(varying)] i32),
    Float(f32),
    String(String),
    Chat(Component),
    OptChat(bool, Option<Component>),
    Slot(Slot),
    Boolean(bool),
    Rotation(f32, f32, f32),
    Position(BlockPosition),
    OptPosition(bool, Option<BlockPosition>),
    Direction(#[packet_serde(varying)] i32),
    OptUuid(bool, Option<Uuid>),
    OptBlockId(#[packet_serde(varying)] i32),
    Nbt(NbtCompound),
    Particle(WrappedParticle),
    VillagerData(i32, i32, i32),
    OptVarInt(#[packet_serde(varying)] i32),
    Pose(#[packet_serde(varying)] i32),
}

#[derive(WriteToPacket)]
pub enum ParticleData {
    AmbientEntityEffect,
    AngryVillager,
    Barrier,
    Block(#[packet_serde(varying)] i32),
    Bubble,
    Cloud,
    Crit,
    DamageIndicator,
    DragonBreath,
    DrippingLava,
    FallingLava,
    LandingLava,
    DrippingWater,
    FallingWater,
    Dust(f32, f32, f32, f32),
    Effect,
    ElderGuardian,
    EnchantedHit,
    Enchant,
    EndRod,
    EntityEffect,
    ExplosionEmitter,
    Explosion,
    FallingDust(#[packet_serde(varying)] i32),
    Firework,
    Fishing,
    Flame,
    Flash,
    HappyVillager,
    Composter,
    Heart,
    InstantEffect,
    Item(Slot),
    ItemSlime,
    ItemSnowball,
    LargeSmoke,
    Lava,
    Mycelium,
    Note,
    Poof,
    Portal,
    Rain,
    Smoke,
    Sneeze,
    Spit,
    SquidInk,
    SweepAttack,
    TotemOfUndying,
    Underwater,
    Splash,
    Witch,
    BubblePop,
    CurrentDown,
    BubbleColumnUp,
    Nautilus,
    Dolphin,
    CampfireCosySmoke,
    CampfireSignalSmoke,
    DrippingHoney,
    FallingHoney,
    LandingHoney,
    FallingNectar,
}

#[derive(WriteToPacket)]
pub enum PlayerInfoAction {
    AddPlayer {
        name: String,
        #[packet_serde(varying)]
        number_of_properties: i32,
        #[packet_serde(len = "number_of_properties")]
        properties: Vec<PlayerProperty>,
        #[packet_serde(varying)]
        gamemode: i32,
        #[packet_serde(varying)]
        ping: i32,
        has_display_name: bool,
        display_name: Option<Component>,
    },
    UpdateGamemode {
        #[packet_serde(varying)]
        gamemode: i32,
    },
    UpdateLatency {
        #[packet_serde(varying)]
        ping: i32,
    },
    UpdateDisplayName {
        has_display_name: bool,
        display_name: Option<Component>,
    },
    RemovePlayer,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct Slot {
    present: bool,
    #[packet_serde(varying, condition = "present")]
    item_id: Option<i32>,
    #[packet_serde(condition = "present")]
    item_count: Option<i8>,
    #[packet_serde(condition = "present")]
    nbt: Option<NbtCompound>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct TabCompleteMatch {
    tab_match: String,
    has_tooltip: bool,
    #[packet_serde(condition = "has_tooltip")]
    tooltip: Option<Component>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct Statistic {
    #[packet_serde(varying)]
    category_id: i32,
    #[packet_serde(varying)]
    statistic_id: i32,
    #[packet_serde(varying)]
    value: i32,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct BlockLights {
    #[packet_serde(varying)]
    pub length: i32,
    #[packet_serde(len = "2048")]
    pub values: Vec<u8>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct MapIcon {
    #[packet_serde(varying)]
    icon_type: i32,
    x: i8,
    z: i8,
    direction: i8,
    has_display_name: bool,
    #[packet_serde(condition = "has_display_name")]
    display_name: Option<Component>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct VillagerTrade {
    input_item_1: Slot,
    output_item: Slot,
    has_second_item: bool,
    #[packet_serde(condition = "has_second_item")]
    input_item_2: Option<Slot>,
    disabled: bool,
    times_used: i32,
    max_uses: i32,
    xp: i32,
    special_price: i32,
    price_multiplier: f32,
    demand: i32,
}

#[derive(WriteToPacket)]
pub struct EntityMetadataWrapper {
    index: u8,
    #[packet_serde(varying, condition = "index != 0xff")]
    var_type: Option<i32>,
    #[packet_serde(condition = "index != 0xff")]
    value: Option<EntityMetadata>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct EquipmentSlot {
    slot: u8,
    item: Slot,
}

impl WriteToPacket for Vec<EquipmentSlot> {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        for (index, slot) in self.iter().enumerate() {
            if index + 1 < self.len() {
                buffer.write_one(slot.slot | 128);
                buffer.write(&slot.item);
            } else {
                buffer.write(slot);
            }
        }
    }
}

impl ReadFromPacket for Vec<EquipmentSlot> {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let mut ret = Vec::new();
        let mut slot: EquipmentSlot;
        loop {
            slot = buffer.read()?;
            let continues = slot.slot > 127;

            if continues {
                slot.slot &= 127;
            }

            ret.push(slot);

            if !continues {
                break;
            }
        }

        Ok(ret)
    }
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AdvancementMapElement {
    key: UnlocalizedName,
    value: Advancement,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AdvancementProgressMapElement {
    key: UnlocalizedName,
    value: AdvancementProgress,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct Advancement {
    has_parent: bool,
    #[packet_serde(condition = "has_parent")]
    parent_id: Option<UnlocalizedName>,
    has_display: bool,
    display_data: AdvancementDisplay,
    #[packet_serde(varying)]
    criteria_len: i32,
    #[packet_serde(len = "criteria_len as usize")]
    criteria: Vec<UnlocalizedName>,
    #[packet_serde(varying)]
    requirements_length: i32,
    #[packet_serde(len = "requirements_length as usize")]
    requirements: Vec<AdvancementRequirements>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AdvancementRequirements {
    #[packet_serde(varying)]
    requirements_len: i32,
    #[packet_serde(len = "requirements_len as usize")]
    requirements: Vec<String>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AdvancementProgress {
    #[packet_serde(varying)]
    size: i32,
    #[packet_serde(len = "size as usize")]
    criteria: Vec<AdvancementProgressCriteria>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AdvancementProgressCriteria {
    identifier: UnlocalizedName,
    progress: CriteriaProgress,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct CriteriaProgress {
    achieved: bool,
    #[packet_serde(condition = "achieved")]
    date_achieved: Option<i64>,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AdvancementDisplay {
    title: Component,
    description: Component,
    icon: Slot,
    #[packet_serde(varying)]
    frame_type: i32,
    flags: i32,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct EntityProperty {
    key: UnlocalizedName,
    value: f64,
    #[packet_serde(varying)]
    number_of_modifiers: i32,
    modifiers: AttributeModifier,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct AttributeModifier {
    uuid: Uuid,
    amount: f64,
    operation: i8,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct Recipe {
    recipe_type: UnlocalizedName,
    recipe_id: String,
    #[packet_serde(greedy)]
    data: Vec<u8>,
}

#[derive(WriteToPacket)]
pub struct WrappedParticle {
    #[packet_serde(varying)]
    id: i32,
    data: ParticleData,
}

#[derive(WriteToPacket, ReadFromPacket)]
pub struct PlayerProperty {
    name: String,
    value: String,
    is_signed: bool,
    #[packet_serde(condition = "is_signed")]
    signature: Option<String>,
}

#[derive(WriteToPacket)]
pub struct WrappedPlayerInfoAction {
    pub uuid: Uuid,
    pub action: PlayerInfoAction,
}

#[derive(WriteToPacket)]
pub struct TagArray {
    #[packet_serde(varying)]
    length: i32,
    data: Vec<Tag>,
}

#[derive(WriteToPacket)]
pub struct Tag {
    name: UnlocalizedName,
    #[packet_serde(varying)]
    count: i32,
    #[packet_serde(varying)]
    entries: Vec<i32>,
}
