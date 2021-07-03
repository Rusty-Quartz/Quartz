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

#[derive(Debug, WriteToPacket)]
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

#[derive(Debug)]
pub struct EntityMetadataWrapper {
    index: u8,
    data: EntityMetadata
}

impl WriteToPacket for Vec<EntityMetadataWrapper> {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        for wrapper in self {
            buffer.write_one(wrapper.index);
            let by = match &wrapper.data {
                EntityMetadata::Byte(_) => 0,
                EntityMetadata::VarInt(_) => 1,
                EntityMetadata::Float(_) => 2,
                EntityMetadata::String(_) => 3,
                EntityMetadata::Chat(_) => 4,
                EntityMetadata::OptChat(..) => 5,
                EntityMetadata::Slot(_) => 6,
                EntityMetadata::Boolean(_) => 7,
                EntityMetadata::Rotation(..) => 8,
                EntityMetadata::Position(_) => 9,
                EntityMetadata::OptPosition(..) => 10,
                EntityMetadata::Direction(_) => 11,
                EntityMetadata::OptUuid(..) => 12,
                EntityMetadata::OptBlockId(_) => 13,
                EntityMetadata::Nbt(_) => 14,
                EntityMetadata::Particle(_) => 15,
                EntityMetadata::VillagerData(..) => 16,
                EntityMetadata::OptVarInt(_) => 17,
                EntityMetadata::Pose(_) => 18,
            };
            buffer.write_one(by);
            buffer.write(&wrapper.data);
        }

        buffer.write_one(0xFF);
    }
}

impl ReadFromPacket for Vec<EntityMetadataWrapper> {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let mut result = Vec::new();

        loop {
            let index = buffer.read_one()?;
            if index == 0xFF {
                return Ok(result);
            }

            let meta_type = buffer.read_one()?;
            let data = match meta_type {
                0 => EntityMetadata::Byte(buffer.read()?),
                1 => EntityMetadata::VarInt(buffer.read_varying()?),
                2 => EntityMetadata::Float(buffer.read()?),
                3 => EntityMetadata::String(buffer.read()?),
                4 => EntityMetadata::Chat(buffer.read()?),
                5 => {
                    let present = buffer.read()?;
                    let component = if present {
                        Some(buffer.read()?)
                    } else {
                        None
                    };
                    EntityMetadata::OptChat(present, component)
                },
                6 => EntityMetadata::Slot(buffer.read()?),
                7 => EntityMetadata::Boolean(buffer.read()?),
                8 => EntityMetadata::Rotation(buffer.read()?, buffer.read()?, buffer.read()?),
                9 => EntityMetadata::Position(buffer.read()?),
                10 => {
                    let present = buffer.read()?;
                    let position = if present {
                        Some(buffer.read()?)
                    } else {
                        None
                    };
                    EntityMetadata::OptPosition(present, position)
                },
                11 => EntityMetadata::Direction(buffer.read_varying()?),
                12 => {
                    let present = buffer.read()?;
                    let uuid = if present {
                        Some(buffer.read()?)
                    } else {
                        None
                    };
                    EntityMetadata::OptUuid(present, uuid)
                },
                13 => EntityMetadata::OptBlockId(buffer.read_varying()?),
                14 => EntityMetadata::Nbt(buffer.read()?),
                15 => EntityMetadata::Particle(buffer.read()?),
                16 => EntityMetadata::VillagerData(buffer.read()?, buffer.read()?, buffer.read()?),
                17 => EntityMetadata::OptVarInt(buffer.read_varying()?),
                18 => EntityMetadata::Pose(buffer.read_varying()?),
                id @ _ => return Err(PacketSerdeError::InvalidEnum("EntityMetadata", id as i32))
            };
            result.push(EntityMetadataWrapper {
                index,
                data
            });
        }
    }
}

#[derive(Debug, WriteToPacket)]
pub enum ParticleData {
    AmbientEntityEffect,
    AngryVillager,
    Barrier,
    Light,
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
    Dust {
        red: f32,
        green: f32,
        blue: f32,
        scale: f32
    },
    DustColorTransition {
        from_red: f32,
        from_green: f32,
        from_blue: f32,
        scale: f32,
        to_red: f32,
        to_green: f32,
        to_blue: f32
    },
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
    SoulFireFlame,
    Soul,
    Flash,
    HappyVillager,
    Composter,
    Heart,
    InstantEffect,
    Item(Slot),
    Vibration {
        origin_x: f64,
        origin_y: f64,
        origin_z: f64,
        dest_x: f64,
        dest_y: f64,
        dest_z: f64,
        ticks: i32
    },
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
    FallingSporeBlossom,
    Ash,
    CrimsonSpore,
    WarpedSpore,
    SporeBlossomAir,
    DrippingObsidianTear,
    FallingObsidianTear,
    LandingObsidianTear,
    ReversePortal,
    WhiteAsh,
    SmallFlame,
    Snowflake,
    DrippingDripstoneLava,
    FallingDripstoneLava,
    DrippingDripstoneWater,
    FallingDripstoneWater,
    GlowSquidInk,
    Glow,
    WaxOn,
    WaxOff,
    ElectricSpark,
    Scrape
}

impl ParticleData {
    pub fn read_particle_data(id: i32, buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let data = match id {
            0 => ParticleData::AmbientEntityEffect,
            1 => ParticleData::AngryVillager,
            2 => ParticleData::Barrier,
            3 => ParticleData::Light,
            4 => ParticleData::Block(buffer.read_varying()?),
            5 => ParticleData::Bubble,
            6 => ParticleData::Cloud,
            7 => ParticleData::Crit,
            8 => ParticleData::DamageIndicator,
            9 => ParticleData::DragonBreath,
            10 => ParticleData::DrippingLava,
            11 => ParticleData::FallingLava,
            12 => ParticleData::LandingLava,
            13 => ParticleData::DrippingWater,
            14 => ParticleData::FallingWater,
            15 => ParticleData::Dust {
                red: buffer.read()?,
                green: buffer.read()?,
                blue: buffer.read()?,
                scale: buffer.read()?
            },
            16 => ParticleData::DustColorTransition {
                from_red: buffer.read()?,
                from_green: buffer.read()?,
                from_blue: buffer.read()?,
                scale: buffer.read()?,
                to_red: buffer.read()?,
                to_green: buffer.read()?,
                to_blue: buffer.read()?
            },
            17 => ParticleData::Effect,
            18 => ParticleData::ElderGuardian,
            19 => ParticleData::EnchantedHit,
            20 => ParticleData::Enchant,
            21 => ParticleData::EndRod,
            22 => ParticleData::EntityEffect,
            23 => ParticleData::ExplosionEmitter,
            24 => ParticleData::Explosion,
            25 => ParticleData::FallingDust(buffer.read_varying()?),
            26 => ParticleData::Firework,
            27 => ParticleData::Fishing,
            28 => ParticleData::Flame,
            29 => ParticleData::SoulFireFlame,
            30 => ParticleData::Soul,
            31 => ParticleData::Flash,
            32 => ParticleData::HappyVillager,
            33 => ParticleData::Composter,
            34 => ParticleData::Heart,
            35 => ParticleData::InstantEffect,
            36 => ParticleData::Item(buffer.read()?),
            37 => ParticleData::Vibration {
                origin_x: buffer.read()?,
                origin_y: buffer.read()?,
                origin_z: buffer.read()?,
                dest_x: buffer.read()?,
                dest_y: buffer.read()?,
                dest_z: buffer.read()?,
                ticks: buffer.read()?,
            },
            38 => ParticleData::ItemSlime,
            39 => ParticleData::ItemSnowball,
            40 => ParticleData::LargeSmoke,
            41 => ParticleData::Lava,
            42 => ParticleData::Mycelium,
            43 => ParticleData::Note,
            44 => ParticleData::Poof,
            45 => ParticleData::Portal,
            46 => ParticleData::Rain,
            47 => ParticleData::Smoke,
            48 => ParticleData::Sneeze,
            49 => ParticleData::Spit,
            50 => ParticleData::SquidInk,
            51 => ParticleData::SweepAttack,
            52 => ParticleData::TotemOfUndying,
            53 => ParticleData::Underwater,
            54 => ParticleData::Splash,
            55 => ParticleData::Witch,
            56 => ParticleData::BubblePop,
            57 => ParticleData::CurrentDown,
            58 => ParticleData::BubbleColumnUp,
            59 => ParticleData::Nautilus,
            60 => ParticleData::Dolphin,
            61 => ParticleData::CampfireCosySmoke,
            62 => ParticleData::CampfireSignalSmoke,
            63 => ParticleData::DrippingHoney,
            64 => ParticleData::FallingHoney,
            65 => ParticleData::LandingHoney,
            66 => ParticleData::FallingNectar,
            67 => ParticleData::FallingSporeBlossom,
            68 => ParticleData::Ash,
            69 => ParticleData::CrimsonSpore,
            70 => ParticleData::WarpedSpore,
            71 => ParticleData::SporeBlossomAir,
            72 => ParticleData::DrippingObsidianTear,
            73 => ParticleData::FallingObsidianTear,
            74 => ParticleData::LandingObsidianTear,
            75 => ParticleData::ReversePortal,
            76 => ParticleData::WhiteAsh,
            77 => ParticleData::SmallFlame,
            78 => ParticleData::Snowflake,
            79 => ParticleData::DrippingDripstoneLava,
            80 => ParticleData::FallingDripstoneLava,
            81 => ParticleData::DrippingDripstoneWater,
            82 => ParticleData::FallingDripstoneWater,
            83 => ParticleData::GlowSquidInk,
            84 => ParticleData::Glow,
            85 => ParticleData::WaxOn,
            86 => ParticleData::WaxOff,
            87 => ParticleData::ElectricSpark,
            88 => ParticleData::Scrape,
            id @ _ => return Err(PacketSerdeError::InvalidEnum("ParticleData", id))
        };

        Ok(data)
    }
}

#[derive(Debug, WriteToPacket)]
pub enum PlayerInfoAction {
    AddPlayer {
        name: String,
        #[packet_serde(varying)]
        number_of_properties: i32,
        #[packet_serde(len = "number_of_properties as usize")]
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

impl PlayerInfoAction {
    pub fn read_action(action: i32, buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        match action {
            0 => {
                let name = buffer.read()?;
                let number_of_properties = buffer.read_varying()?;
                let properties = buffer.read_array(number_of_properties as usize)?;
                let gamemode = buffer.read_varying()?;
                let ping = buffer.read_varying()?;
                let has_display_name = buffer.read()?;
                let display_name = if has_display_name {
                    Some(buffer.read()?)
                } else {
                    None
                };

                Ok(PlayerInfoAction::AddPlayer {
                    name,
                    number_of_properties,
                    properties,
                    gamemode,
                    ping,
                    has_display_name,
                    display_name
                })
            },
            1 => {
                let gamemode = buffer.read_varying()?;

                Ok(PlayerInfoAction::UpdateGamemode {
                    gamemode
                })
            },
            2 => {
                let ping = buffer.read_varying()?;

                Ok(PlayerInfoAction::UpdateLatency {
                    ping
                })
            },
            3 => {
                let has_display_name = buffer.read()?;
                let display_name = if has_display_name {
                    Some(buffer.read()?)
                } else {
                    None
                };

                Ok(PlayerInfoAction::UpdateDisplayName {
                    has_display_name,
                    display_name
                })
            },
            id @ _ => Err(PacketSerdeError::InvalidEnum("PlayerInfoAction", id))
        }
    }
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct Slot {
    present: bool,
    #[packet_serde(varying, condition = "present")]
    item_id: Option<i32>,
    #[packet_serde(condition = "present")]
    item_count: Option<i8>,
    #[packet_serde(condition = "present")]
    nbt: Option<NbtCompound>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct TabCompleteMatch {
    tab_match: String,
    has_tooltip: bool,
    #[packet_serde(condition = "has_tooltip")]
    tooltip: Option<Component>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct Statistic {
    #[packet_serde(varying)]
    category_id: i32,
    #[packet_serde(varying)]
    statistic_id: i32,
    #[packet_serde(varying)]
    value: i32,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct BlockLights {
    #[packet_serde(varying)]
    pub length: i32,
    #[packet_serde(len = "2048")]
    pub values: Vec<u8>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
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

#[derive(Debug, WriteToPacket, ReadFromPacket)]
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

#[derive(Debug, WriteToPacket, ReadFromPacket)]
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

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AdvancementMapElement {
    key: UnlocalizedName,
    value: Advancement,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AdvancementProgressMapElement {
    key: UnlocalizedName,
    value: AdvancementProgress,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
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

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AdvancementRequirements {
    #[packet_serde(varying)]
    requirements_len: i32,
    #[packet_serde(len = "requirements_len as usize")]
    requirements: Vec<String>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AdvancementProgress {
    #[packet_serde(varying)]
    size: i32,
    #[packet_serde(len = "size as usize")]
    criteria: Vec<AdvancementProgressCriteria>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AdvancementProgressCriteria {
    identifier: UnlocalizedName,
    progress: CriteriaProgress,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct CriteriaProgress {
    achieved: bool,
    #[packet_serde(condition = "achieved")]
    date_achieved: Option<i64>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AdvancementDisplay {
    title: Component,
    description: Component,
    icon: Slot,
    #[packet_serde(varying)]
    frame_type: i32,
    flags: i32,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct EntityProperty {
    key: UnlocalizedName,
    value: f64,
    #[packet_serde(varying)]
    number_of_modifiers: i32,
    modifiers: AttributeModifier,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct AttributeModifier {
    uuid: Uuid,
    amount: f64,
    operation: i8,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct Recipe {
    recipe_type: UnlocalizedName,
    recipe_id: String,
    #[packet_serde(greedy)]
    data: Vec<u8>,
}

#[derive(Debug, WriteToPacket)]
pub struct WrappedParticle {
    #[packet_serde(varying)]
    id: i32,
    data: ParticleData,
}

impl ReadFromPacket for WrappedParticle {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let id = buffer.read()?;
        let data = ParticleData::read_particle_data(id, buffer)?;
        Ok(WrappedParticle {
            id,
            data
        })
    }
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct PlayerProperty {
    name: String,
    value: String,
    is_signed: bool,
    #[packet_serde(condition = "is_signed")]
    signature: Option<String>,
}

#[derive(Debug, WriteToPacket)]
pub struct WrappedPlayerInfoAction {
    pub uuid: Uuid,
    pub action: PlayerInfoAction,
}

impl WrappedPlayerInfoAction {
    pub fn read_action(action: i32, buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let uuid = buffer.read()?;
        let action = PlayerInfoAction::read_action(action, buffer)?;
        Ok(WrappedPlayerInfoAction {
            uuid,
            action
        })
    }
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct TagArray {
    #[packet_serde(varying)]
    length: i32,
    #[packet_serde(len = "length as usize")]
    data: Vec<Tag>,
}

#[derive(Debug, WriteToPacket, ReadFromPacket)]
pub struct Tag {
    name: UnlocalizedName,
    #[packet_serde(varying)]
    count: i32,
    #[packet_serde(len = "count as usize", varying)]
    entries: Vec<i32>,
}
