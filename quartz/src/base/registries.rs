use std::{collections::HashMap, path::PathBuf};

use flashmap::{new as new_flashmap, ReadGuard, ReadHandle, View, WriteHandle};
use qdat::UnlocalizedName;
use quartz_datapack::{
    data::{
        advancement::Advancement,
        biome::Biome,
        carvers::Carver,
        density_function::DensityFunctionProvider,
        dimension_type::DimensionType,
        features::{Feature, PlacedFeature},
        functions::Function,
        item_modifiers::ItemModifier,
        jigsaw_pool::JigsawPool,
        loot_tables::LootTable,
        noise::Noise,
        noise_settings::NoiseSettings,
        predicate::Predicate,
        processors::ProcessorList,
        recipe::VanillaRecipeType,
        structure::Structure,
        structure_features::StructureFeatures,
        structure_set::StructureSet,
        surface_builders::SurfaceBuilder,
        tags::Tag,
    },
    DataPack,
    VersionFilter,
};

use crate::world::world::Dimension;

/// Provides a central access to all things loaded from datapacks
///
/// These are centralized to make reloading datapacks relatively easy
///
/// We also don't just store [datapacks](DataPack) because some types can be condensed and / or optimized
///
/// Ex/ Density Functions can be compiled down into a format that is quicker to execute due to lookups being done at load
// TODO: actually implement the special types for these
#[derive(Default)]
pub struct Registries {
    pub tags: Registry<Tag>,
    pub recipes: Registry<VanillaRecipeType>,
    pub functions: Registry<Function>,
    pub loot_tables: Registry<LootTable>,
    pub predicates: Registry<Predicate>,
    pub item_modifiers: Registry<ItemModifier>,
    pub advancements: Registry<Advancement>,
    pub biomes: Registry<Biome>,
    pub density_functions: Registry<DensityFunctionProvider>,
    pub dimensions: Registry<Dimension>,
    pub dimension_types: Registry<DimensionType>,
    pub noise: Registry<Noise>,
    pub noise_settings: Registry<NoiseSettings>,
    pub carvers: Registry<Carver>,
    pub surface_builders: Registry<SurfaceBuilder>,
    pub features: Registry<Feature>,
    pub structure_features: Registry<StructureFeatures>,
    pub jigsaw_pools: Registry<JigsawPool>,
    pub processors: Registry<ProcessorList>,
    pub structure_sets: Registry<StructureSet>,
    pub structures: Registry<Structure>,
    pub placed_features: Registry<PlacedFeature>,
}

macro_rules! registry_datapack_load {
    ($registries: ident, $namespace: ident, $($field_name: ident),*) => {
        $(
            $registries
                    .$field_name
                    .insert_all($namespace.$field_name.into_iter().map(|(k, v)| {
                        (
                            // this should be safe to unwrap
                            // name and k can't be empty cause they come from filenames
                            // the only possible error could be if the uln is to long
                            UnlocalizedName::from_parts(&$namespace.name, &k)
                                .unwrap(),
                            v,
                        )
                    }));
        )*
    };
}

impl Registries {
    pub fn load(datapacks_dir: &impl AsRef<PathBuf>) -> Result<Registries, RegistryError> {
        let datapacks_dir = datapacks_dir.as_ref();

        if !datapacks_dir.is_dir() && !datapacks_dir.exists() {
            return Err(RegistryError::InvalidDatapackDirectory(
                datapacks_dir.clone(),
            ));
        }

        let packs = match DataPack::read_datapacks(&datapacks_dir, VersionFilter::LatestOrStable) {
            Ok(packs) => packs,
            Err(e) => return Err(RegistryError::ReadError(e)),
        };

        let packs = packs
            .into_iter()
            .filter_map(|pack| match pack {
                Ok(pack) => Some(pack),
                Err(e) => {
                    log::error!("Error loading datapack: {e}");
                    None
                }
            })
            .collect::<Vec<_>>();

        if packs.is_empty() {
            return Err(RegistryError::NoValidDatapacks);
        }

        let registries = Registries::from_datapacks(packs);

        registries.validate();

        Ok(registries)
    }

    /// Loads the datapacks provided into a new registry collection
    fn from_datapacks(datapacks: Vec<DataPack>) -> Registries {
        let mut registries: Registries = Default::default();

        for pack in datapacks {
            for namespace in pack.namespaces {
                registries
                    .tags
                    .insert_all(namespace.tags.into_iter().map(|t| {
                        (
                            UnlocalizedName::from_parts(&namespace.name, &t.name).unwrap(),
                            t,
                        )
                    }));

                registry_datapack_load!(
                    registries,
                    namespace,
                    recipes,
                    functions,
                    loot_tables,
                    predicates,
                    item_modifiers,
                    advancements,
                    biomes,
                    density_functions,
                    dimension_types,
                    noise,
                    noise_settings,
                    carvers,
                    surface_builders,
                    features,
                    structure_features,
                    jigsaw_pools,
                    processors,
                    structure_sets,
                    structures,
                    placed_features
                );
            }
        }

        registries
    }

    /// Validates that all the data loaded in is valid
    ///
    /// This will traverse the structures and make sure things like references are loaded and constants are in range
    fn validate(&self) {}

    // I don't think vanilla lets you reload the default datapack
    // but we need to in order for the new filters to work I think
    // also it would be a mess to store it differently
    pub fn reload(&mut self) {}
}

pub struct Registry<T> {
    read_handle: ReadHandle<UnlocalizedName, T>,
    write_handle: WriteHandle<UnlocalizedName, T>,
}

impl<T> Registry<T> {
    fn empty() -> Self {
        let (write, read) = new_flashmap();
        Registry {
            read_handle: read,
            write_handle: write,
        }
    }

    #[allow(unused)]
    fn new(map: impl IntoIterator<Item = (UnlocalizedName, T)>) -> Registry<T> {
        let (mut write, read) = new_flashmap();

        let mut write_guard = write.guard();

        for (key, val) in map {
            write_guard.insert(key, val);
        }

        drop(write_guard);
        Registry {
            read_handle: read,
            write_handle: write,
        }
    }

    /// Returns a read handle, which allows the creation of multiple read guards to the registry
    pub fn get_read_handle(&self) -> RegistryHandle<T> {
        RegistryHandle {
            read_handle: self.read_handle.clone(),
        }
    }

    /// Inserts the key value pair into the registry
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn insert(&mut self, key: UnlocalizedName, value: T) {
        let mut write_guard = self.write_handle.guard();

        write_guard.insert(key, value);
    }

    /// Inserts all the key value pair into the registry
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn insert_all(&mut self, entries: impl IntoIterator<Item = (UnlocalizedName, T)>) {
        let mut write_guard = self.write_handle.guard();

        for (key, value) in entries {
            write_guard.insert(key, value);
        }
    }

    /// Clears the registry
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn clear(&mut self) {
        let mut write_guard = self.write_handle.guard();

        // This kinda sucks, but this method is not going to be used much if at all
        let keys = write_guard.keys().cloned().collect::<Vec<_>>();

        for key in keys {
            write_guard.remove(key);
        }
    }

    /// Replaces the current data with the data stored in `map`
    ///
    /// Existing read guards won't see the change until you create a new guard
    pub fn replace_map(&mut self, map: HashMap<UnlocalizedName, T>) {
        let mut write_guard = self.write_handle.guard();

        // I don't know if the filter call is slower than just cloning the whole map
        // I think hashing UnlocalizedName is faster than allocating mem but im not sure
        let to_remove = write_guard
            .keys()
            .filter(|k| !map.contains_key(*k))
            .cloned()
            .collect::<Vec<_>>();

        for key in to_remove {
            write_guard.remove(key);
        }

        for (key, value) in map {
            write_guard.insert(key, value);
        }
    }
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Registry::<T>::empty()
    }
}

/// A handle to a Registry, allows the creation of read guards to read a snapshot of the data in the registry   
pub struct RegistryHandle<T> {
    read_handle: ReadHandle<UnlocalizedName, T>,
}

impl<T> RegistryHandle<T> {
    pub fn read(&self) -> View<ReadGuard<'_, UnlocalizedName, T>> {
        self.read_handle.guard()
    }
}


pub enum RegistryError {
    InvalidDatapackDirectory(PathBuf),
    ReadError(quartz_datapack::DatapackIoError),
    NoValidDatapacks,
}
