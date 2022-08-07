use std::{
    collections::HashMap,
    fs::{DirEntry, File, OpenOptions},
    io::{Error, ErrorKind, Read, Result},
    path::Path,
};

use quartz_nbt::io::{Flavor, NbtIoError};
use serde::{Deserialize, Serialize};

use crate::data::{
    advancement::Advancement,
    biome::Biome,
    carvers::Carver,
    density_function::DensityFunctionProvider,
    dimension::Dimension,
    dimension_type::DimensionType,
    features::{Feature, PlacedFeature},
    functions::{read_function, write_function, Function},
    item_modifiers::ItemModifier,
    jigsaw_pool::JigsawPool,
    loot_tables::LootTable,
    noise_settings::NoiseSettings,
    predicate::Predicate,
    processors::ProcessorList,
    recipe::VanillaRecipeType,
    structure::Structure,
    structure_features::StructureFeatures,
    structure_set::StructureSet,
    surface_builders::SurfaceBuilder,
    tags::Tag,
};

/// Gets the datapack version for the minecraft version
///
/// Returns 0 if the version does not support datapacks<br>
/// Returns 1 if the version is not out yet / we don't support it yet
///
/// Versions 1.13-1.16.5 should mostly be supported as datapacks are mostly backwards compatable, though a some things have changed which makes some features incompatable<br>
/// For incompatabilities check the minecraft wiki
///
/// For snapshot versions use the version it is a snapshot for<br>
/// Ex: for 21w03a you would use 1.17.0
pub const fn datapack_version(major: u8, minor: u8, patch: u8) -> u8 {
    match (major, minor, patch) {
        // Any version before 1.13
        (0, ..) | (1, 0 ..= 12, _) => 0,
        // The versions currently with datapacks
        (1, 13 | 14, _) => 4,
        (1, 15, _) | (1, 16, 0 | 1) => 5,
        (1, 16, _) => 6,
        (1, 17, _) => 7,
        (1, 18, 2) => 9,
        (1, 18, _) => 8,
        (1, 19, _) => 10,
        // Future versions we don't support
        _ => 1,
    }
}
/// The datapack version we currently support
///
/// This takes into account experimental features and so will be the strictest possible filter for supported versions
pub const SUPPORTED_VERSION: u8 = 9;
/// The lowest datapack version we support if the pack is not using experimental features
///
/// The currently the only experimental features are the worldgen features
// TODO: actually check that this is accurate 1.16 feels right because that was before the worldgen stuff was added iirc
pub const LOWEST_SUPPORTED_STABLE_VERSION: u8 = 6;

#[derive(Clone, Copy)]
pub enum VersionFilter {
    /// Only allow pack formats that are equal to [SUPPORTED_VERSION]
    Latest,
    /// Allow pack formats that are equal to or greater than [LOWEST_SUPPORTED_STABLE_VERSION]
    Stable,
    /// Same as [Latest](VersionFilter::Latest) unless the pack does not contain any experimental features, then allow [Stable](VersionFilter::Stable)
    LatestOrStable,
    /// Try to load all packs and ignore the given pack format
    None,
}

fn recursive_read(path: &Path, prefix: String) -> Result<Vec<(String, DirEntry)>> {
    let mut entries = Vec::new();

    for entry in path.read_dir()? {
        let entry = entry?;

        if entry.metadata()?.is_dir() {
            entries.extend(recursive_read(
                &entry.path(),
                format!(
                    "{}{}/",
                    prefix,
                    entry.file_name().to_string_lossy().to_string()
                ),
            )?);
        } else {
            entries.push((
                format!(
                    "{}{}",
                    prefix,
                    entry
                        .file_name()
                        .to_string_lossy()
                        .to_string()
                        .split('.')
                        .next()
                        .unwrap()
                ),
                entry,
            ));
        }
    }

    Ok(entries)
}

fn write_file_recursive<P: AsRef<Path>>(path: P) -> Result<File> {
    // TODO: This unwrap technically could fail if someone tried to write a datapack to the rootdir I think
    std::fs::create_dir_all(path.as_ref().parent().unwrap())?;
    OpenOptions::new()
        .read(false)
        .write(true)
        .create(true)
        .open(path)
}

/// Holds all the info about the datapack
pub struct DataPack {
    pub meta: McMeta,
    pub namespaces: Vec<Namespace>,
}

impl DataPack {
    /// Gets the version of the datapack format we are using
    pub fn version(&self) -> u8 {
        self.meta.pack_format
    }

    /// Gets the description of the datapack
    pub fn description(&self) -> &str {
        &self.meta.description
    }

    /// Gets the name of the datapack
    pub fn name(&self) -> &str {
        &self.meta.name
    }

    /// Reads in all the datapacks in a directory
    ///
    /// `version_filter` allows you to provide a filter for which pack formats will be attempted to be loaded
    pub fn read_datapacks<P: AsRef<Path>>(
        path: &P,
        version_filter: VersionFilter,
    ) -> Result<Vec<Result<DataPack>>> {
        let files = path.as_ref().read_dir()?;
        let mut packs = Vec::new();

        for entry in files {
            let entry = entry?;

            if entry.metadata()?.is_dir() {
                packs.push(Self::read(
                    &entry.path(),
                    entry.file_name().to_str().unwrap(),
                    version_filter,
                ))
            }
        }

        Ok(packs)
    }

    /// Reads in a datapack from a given folder.
    ///
    /// `version_filter` allows you to provide a filter for which pack formats will be attempted to be loaded
    pub fn read(path: &Path, pack_name: &str, version_filter: VersionFilter) -> Result<DataPack> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(false)
            .open(path.join("pack.mcmeta"))?;

        let mut json = String::new();
        file.read_to_string(&mut json)?;

        let mut meta: RawMcMeta = serde_json::from_str(&json)?;

        match version_filter {
            VersionFilter::Latest =>
                if meta.pack.pack_format != SUPPORTED_VERSION {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Pack {} format version ({}) is lower than the supported version",
                            meta.pack.name, meta.pack.pack_format
                        ),
                    ));
                },
            VersionFilter::Stable =>
                if meta.pack.pack_format < LOWEST_SUPPORTED_STABLE_VERSION {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Pack {} format version ({}) is lower than the supported version",
                            meta.pack.name, meta.pack.pack_format
                        ),
                    ));
                },
            VersionFilter::LatestOrStable =>
                if meta.pack.pack_format != SUPPORTED_VERSION {
                    if meta.pack.pack_format < LOWEST_SUPPORTED_STABLE_VERSION
                        || path.join("data/worldgen").exists()
                    {
                        return Err(Error::new(
                            ErrorKind::Other,
                            format!(
                                "Pack {} format version ({}) is lower than the supported version",
                                meta.pack.name, meta.pack.pack_format
                            ),
                        ));
                    }
                },
            VersionFilter::None => {}
        }


        meta.pack.name = pack_name.to_owned();

        Ok(DataPack {
            meta: meta.pack,
            namespaces: Self::read_namespaces(&path.join("data"))?,
        })
    }

    fn read_namespaces(data_path: &Path) -> Result<Vec<Namespace>> {
        let mut namespaces = Vec::new();

        for entry in (data_path.read_dir()?).flatten() {
            if entry.metadata()?.is_dir() {
                namespaces.push(Namespace::read(&entry.path())?)
            }
        }

        Ok(namespaces)
    }

    pub fn write_datapack<P: AsRef<Path>>(&self, path: &P) -> Result<()> {
        let mcmeta_file = write_file_recursive(path.as_ref().join("pack.mcmeta"))?;
        serde_json::to_writer(mcmeta_file, &self.meta)?;


        for namespace in &self.namespaces {
            namespace.write(&path.as_ref().join(&namespace.name))?;
        }
        Ok(())
    }
}

pub struct Namespace {
    pub name: String,
    pub tags: Vec<Tag>,
    pub recipes: HashMap<String, VanillaRecipeType>,
    pub functions: HashMap<String, Function>,
    pub loot_tables: HashMap<String, LootTable>,
    pub predicates: HashMap<String, Predicate>,
    pub item_modifiers: HashMap<String, ItemModifier>,
    pub advancements: HashMap<String, Advancement>,
    pub biomes: HashMap<String, Biome>,
    pub density_functions: HashMap<String, DensityFunctionProvider>,
    pub dimensions: HashMap<String, Dimension>,
    pub dimension_types: HashMap<String, DimensionType>,
    pub noise_settings: HashMap<String, NoiseSettings>,
    pub carvers: HashMap<String, Carver>,
    pub surface_builders: HashMap<String, SurfaceBuilder>,
    pub features: HashMap<String, Feature>,
    pub structure_features: HashMap<String, StructureFeatures>,
    pub jigsaw_pools: HashMap<String, JigsawPool>,
    pub processors: HashMap<String, ProcessorList>,
    pub structure_sets: HashMap<String, StructureSet>,
    pub structures: HashMap<String, Structure>,
    pub placed_features: HashMap<String, PlacedFeature>,
}

impl Namespace {
    fn read(namespace_path: &Path) -> Result<Namespace> {
        // will only fail if the datapack has a file / folder whose name ends with '..' which, to my knowledge is not possible
        let name = namespace_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let tags = Self::read_tags(&namespace_path.join("tags"))?;
        let recipes = Self::read_datatype(&namespace_path.join("recipes"))?;
        let advancements = Self::read_datatype(&namespace_path.join("advancements"))?;
        let functions = Self::read_functions(&namespace_path.join("functions"))?;
        let loot_tables = Self::read_datatype(&namespace_path.join("loot_tables"))?;
        let predicates = Self::read_datatype(&namespace_path.join("predicates"))?;
        let item_modifiers = Self::read_datatype(&namespace_path.join("item_modifiers"))?;
        let dimensions = Self::read_datatype(&namespace_path.join("dimension"))?;
        let dimension_types = Self::read_datatype(&namespace_path.join("dimension_type"))?;
        let biomes = Self::read_datatype(&namespace_path.join("worldgen/biome"))?;
        let carvers = Self::read_datatype(&namespace_path.join("worldgen/configured_carver"))?;
        let features = Self::read_datatype(&namespace_path.join("worldgen/configured_feature"))?;
        let structure_features =
            Self::read_datatype(&namespace_path.join("worldgen/configured_structure_feature"))?;
        let surface_builders =
            Self::read_datatype(&namespace_path.join("worldgen/configured_surface_builder"))?;
        let noise_settings = Self::read_datatype(&namespace_path.join("worldgen/noise_settings"))?;
        let processors = Self::read_datatype(&namespace_path.join("worldgen/processor_list"))?;
        let jigsaw_pools = Self::read_datatype(&namespace_path.join("worldgen/template_pool"))?;
        let density_functions =
            Self::read_datatype(&namespace_path.join("worldgen/density_function"))?;
        let placed_features = Self::read_datatype(&namespace_path.join("worldgen/placed_feature"))?;
        let structure_sets = Self::read_datatype(&namespace_path.join("worldgen/structure_set"))?;
        let structures = match Self::read_structures(&namespace_path.join("structures")) {
            Ok(s) => s,
            Err(e) =>
            // I don't feel like changing error types to make this better
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("{}", e),
                )),
        };

        Ok(Namespace {
            name,
            tags,
            recipes,
            advancements,
            functions,
            loot_tables,
            predicates,
            item_modifiers,
            density_functions,
            dimensions,
            dimension_types,
            biomes,
            carvers,
            features,
            structure_features,
            surface_builders,
            noise_settings,
            processors,
            jigsaw_pools,
            structures,
            structure_sets,
            placed_features,
        })
    }

    fn read_tags(tags_path: &Path) -> Result<Vec<Tag>> {
        let mut tags = Vec::new();

        let tag_files = match recursive_read(tags_path, String::new()) {
            Ok(e) => e,
            Err(_) => return Ok(tags),
        };

        for (name, entry) in tag_files {
            println!("{name} {entry:?}");
            let mut file = OpenOptions::new()
                .read(true)
                .write(false)
                .open(entry.path())?;

            let mut string = String::new();
            file.read_to_string(&mut string)?;

            let tag = match serde_json::from_str(&string) {
                Ok(v) => v,
                Err(e) => {
                    return Err(e.into());
                }
            };

            tags.push(Tag { name, def: tag })
        }

        Ok(tags)
    }

    fn read_functions(functions_path: &Path) -> Result<HashMap<String, Function>> {
        let mut functions = HashMap::new();

        let files = match recursive_read(functions_path, String::new()) {
            Ok(e) => e,
            Err(_) => return Ok(functions),
        };

        for (name, entry) in files {
            println!("{name} {entry:?}");
            let file = OpenOptions::new()
                .read(true)
                .write(false)
                .open(entry.path())?;

            functions.insert(name, read_function(file)?);
        }

        Ok(functions)
    }

    fn read_structures(
        functions_path: &Path,
    ) -> std::result::Result<HashMap<String, Structure>, NbtIoError> {
        let mut structures = HashMap::new();

        let files = match recursive_read(functions_path, String::new()) {
            Ok(e) => e,
            Err(_) => return Ok(structures),
        };

        for (name, entry) in files {
            println!("{name} {entry:?}");
            let mut file = OpenOptions::new()
                .read(true)
                .write(false)
                .open(entry.path())
                .map_err(NbtIoError::StdIo)?;

            // I'm pretty sure this is umcompressed
            structures.insert(
                name,
                quartz_nbt::serde::deserialize_from(&mut file, Flavor::GzCompressed)?.0,
            );
        }

        Ok(structures)
    }

    fn read_datatype<T: for<'de> Deserialize<'de>>(data_path: &Path) -> Result<HashMap<String, T>> {
        let mut output = HashMap::new();
        let files = match recursive_read(data_path, String::new()) {
            Ok(e) => e,
            Err(_) => return Ok(output),
        };

        for (name, entry) in files {
            println!("{name} {entry:?}");
            let mut file = OpenOptions::new()
                .read(true)
                .write(false)
                .open(entry.path())?;

            let mut str = String::new();
            file.read_to_string(&mut str)?;

            let data = match serde_json::from_str(&str) {
                Ok(v) => v,
                Err(e) => {
                    return Err(e.into());
                }
            };
            output.insert(name, data);
        }

        Ok(output)
    }

    fn write<P: AsRef<Path>>(&self, namespace_path: &P) -> Result<()> {
        let namespace_path = namespace_path.as_ref();

        Self::write_tags(&self.tags, &namespace_path.join("tags"))?;
        Self::write_datatype(&self.recipes, &namespace_path.join("recipes"))?;
        Self::write_datatype(&self.advancements, &namespace_path.join("advancements"))?;
        Self::write_functions(&self.functions, &namespace_path.join("functions"))?;
        Self::write_datatype(&self.loot_tables, &namespace_path.join("loot_tables"))?;
        Self::write_datatype(&self.predicates, &namespace_path.join("predicates"))?;
        Self::write_datatype(&self.item_modifiers, &namespace_path.join("item_modifiers"))?;
        Self::write_datatype(&self.dimensions, &namespace_path.join("dimension"))?;
        Self::write_datatype(
            &self.dimension_types,
            &namespace_path.join("dimension_type"),
        )?;
        Self::write_datatype(&self.biomes, &namespace_path.join("worldgen/biome"))?;
        Self::write_datatype(
            &self.carvers,
            &namespace_path.join("worldgen/configured_carver"),
        )?;
        Self::write_datatype(
            &self.features,
            &namespace_path.join("worldgen/configured_feature"),
        )?;

        Self::write_datatype(
            &self.structure_features,
            &namespace_path.join("worldgen/configured_structure_feature"),
        )?;

        Self::write_datatype(
            &self.surface_builders,
            &namespace_path.join("worldgen/configured_surface_builder"),
        )?;
        Self::write_datatype(
            &self.noise_settings,
            &namespace_path.join("worldgen/noise_settings"),
        )?;
        Self::write_datatype(
            &self.processors,
            &namespace_path.join("worldgen/processor_list"),
        )?;
        Self::write_datatype(
            &self.jigsaw_pools,
            &namespace_path.join("worldgen/template_pool"),
        )?;
        Self::write_datatype(
            &self.density_functions,
            &namespace_path.join("worldgen/density_function"),
        )?;
        Self::write_datatype(
            &self.placed_features,
            &namespace_path.join("worldgen/placed_feature"),
        )?;
        Self::write_datatype(
            &self.structure_sets,
            &namespace_path.join("worldgen/structure_set"),
        )?;
        match Self::write_structures(&self.structures, &namespace_path.join("structures")) {
            Ok(_) => {}
            Err(e) =>
            // I don't feel like changing error types to make this better
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("{}", e),
                )),
        };

        Ok(())
    }

    fn write_tags<P: AsRef<Path>>(tags: &Vec<Tag>, path: &P) -> Result<()> {
        for tag in tags {
            let file = write_file_recursive(path.as_ref().join(format!("{}.json", tag.name())))?;

            serde_json::to_writer(file, &tag.def)?;
        }
        Ok(())
    }

    fn write_datatype<T: Serialize, P: AsRef<Path>>(
        value: &HashMap<String, T>,
        path: &P,
    ) -> Result<()> {
        for (name, val) in value {
            let file = write_file_recursive(path.as_ref().join(format!("{}.json", name)))?;

            serde_json::to_writer(file, val)?;
        }
        Ok(())
    }

    fn write_structures<P: AsRef<Path>>(
        structures: &HashMap<String, Structure>,
        path: &P,
    ) -> std::result::Result<(), NbtIoError> {
        for (name, val) in structures {
            let mut file = write_file_recursive(path.as_ref().join(format!("{}.nbt", name)))?;

            quartz_nbt::serde::serialize_into(&mut file, val, None, Flavor::GzCompressed)?;
        }
        Ok(())
    }

    fn write_functions<P: AsRef<Path>>(
        functions: &HashMap<String, Function>,
        path: &P,
    ) -> Result<()> {
        for (name, func) in functions {
            let file = OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .open(path.as_ref().join(format!("{}.mcfunction", name)))?;

            write_function(func, file)?;
        }
        Ok(())
    }
}
/// Holds the metadata about the pack
///
/// Does not directly represent the format of the `pack.mcmeta` file because we also store the name of the pack
#[derive(Serialize, Deserialize)]
pub struct McMeta {
    pub pack_format: u8,
    pub description: String,
    #[serde(skip)]
    pub name: String,
}

/// Represents the actual format of the `pack.mcmeta` file
///
/// Only needed because the actual data of mcmeta is wrapped in the `pack` field
// Mojang why do you not have the data in the root
#[derive(Serialize, Deserialize)]
struct RawMcMeta {
    pub pack: McMeta,
}
