use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::Path,
    sync::Arc,
};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use hecs::{Bundle, Entity, World as EntityStore};

use qdat::{world::location::Coordinate, UnlocalizedName};

use crate::{
    entities::player::Player,
    network::AsyncWriteHandle,
    server::ClientId,
    world::chunk::{
        provider::{MapRef, MapRefMut, ProviderRequest},
        Chunk,
        ChunkProvider,
    },
};

pub struct World {
    entities: Arc<RwLock<EntityStore>>,
    curr_players: HashMap<ClientId, Entity>,
    chunk_provider: ChunkProvider,
}

// Player {...}
// (Inventroy, Position, Gamemode)

impl World {
    fn new<P: AsRef<Path>>(name: &str, world_path: P) -> std::io::Result<Self> {
        let entities = Arc::new(RwLock::new(EntityStore::new()));
        let curr_players = HashMap::new();
        let chunk_provider = ChunkProvider::new(name, world_path)?;

        Ok(Self {
            entities,
            curr_players,
            chunk_provider,
        })
    }

    /// Spawns a player entity and returns the entity handle
    ///
    /// Does not load the chunks around the player
    pub async fn spawn_player(&mut self, player_id: ClientId, player: Player) -> Entity {
        let mut entities = self.entities.write().await;
        // TODO: actually spawn a player here
        let player = entities.spawn(player);
        self.curr_players.insert(player_id, player);
        player
    }

    pub fn get_player_entity(&self, player_id: ClientId) -> Option<&Entity> {
        self.curr_players.get(&player_id)
    }

    pub async fn spawn_entity<E: Bundle>(&mut self, entity: E) -> Entity {
        let mut entities = self.entities.write().await;
        entities.spawn(entity)
    }

    /// Returns an exclusive reference to the entity store
    pub async fn get_entities_mut(&mut self) -> EntitiesRefMut<'_> {
        EntitiesRefMut {
            lock: self.entities.write().await,
        }
    }

    /// Returns a reference to the entity store
    pub async fn get_entities(&mut self) -> EntitiesRef<'_> {
        EntitiesRef {
            lock: self.entities.read().await,
        }
    }

    pub fn load_chunk(&self, coords: Coordinate) {
        self.chunk_provider
            .request(ProviderRequest::LoadFull(coords))
    }

    pub fn load_send_chunk(&self, coords: Coordinate, handle: AsyncWriteHandle) {
        self.chunk_provider
            .request(ProviderRequest::MinLoadSend { coords, handle })
    }

    pub fn unload_chunk(&self, coords: Coordinate) {
        self.chunk_provider.request(ProviderRequest::Unload(coords))
    }

    pub fn get_loaded_chunk(&self, coords: Coordinate) -> Option<MapRef<'_, Chunk>> {
        self.chunk_provider.store.loaded_chunk_at(coords)
    }

    pub fn get_loaded_chunk_mut(&mut self, coords: Coordinate) -> Option<MapRefMut<'_, Chunk>> {
        self.chunk_provider.store.loaded_chunk_at_mut(coords)
    }

    pub async fn join_pending(&mut self) {
        self.chunk_provider.join_pending().await;
    }
}

pub struct EntitiesRef<'a> {
    lock: RwLockReadGuard<'a, EntityStore>,
}

impl<'a> Deref for EntitiesRef<'a> {
    type Target = EntityStore;

    fn deref(&self) -> &Self::Target {
        self.lock.deref()
    }
}

pub struct EntitiesRefMut<'a> {
    lock: RwLockWriteGuard<'a, EntityStore>,
}

impl<'a> Deref for EntitiesRefMut<'a> {
    type Target = EntityStore;

    fn deref(&self) -> &Self::Target {
        self.lock.deref()
    }
}

impl<'a> DerefMut for EntitiesRefMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.lock.deref_mut()
    }
}

pub struct WorldStore {
    worlds: HashMap<Dimension, World>,
    /// Stores which world each player is currently in
    player_worlds: HashMap<ClientId, Dimension>,
}

impl WorldStore {
    pub async fn spawn_player(
        &mut self,
        dim: Dimension,
        player_id: ClientId,
        player: Player,
    ) -> Option<Entity> {
        self.player_worlds.insert(player_id, dim.clone());
        Some(
            self.worlds
                .get_mut(&dim)?
                .spawn_player(player_id, player)
                .await,
        )
    }

    pub fn get_player_world(&self, player_id: ClientId) -> Option<&World> {
        let world_id = self.player_worlds.get(&player_id)?;
        self.worlds.get(world_id)
    }

    pub fn get_player_world_mut(&mut self, player_id: ClientId) -> Option<&mut World> {
        let world_id = self.player_worlds.get(&player_id)?;
        self.worlds.get_mut(world_id)
    }

    pub fn get_world(&self, dim: Dimension) -> Option<&World> {
        self.worlds.get(&dim)
    }

    pub fn get_world_mut(&mut self, dim: Dimension) -> Option<&mut World> {
        self.worlds.get_mut(&dim)
    }

    pub fn new<P: AsRef<Path>>(world_path: P) -> std::io::Result<Self> {
        let mut worlds = HashMap::with_capacity(3);
        let player_worlds = HashMap::new();

        worlds.insert(
            Dimension::Overworld,
            World::new("overworld", world_path.as_ref().join("region"))?,
        );

        worlds.insert(
            Dimension::Nether,
            World::new("nether", world_path.as_ref().join("DIM-1/region"))?,
        );

        worlds.insert(
            Dimension::End,
            World::new("the end", world_path.as_ref().join("DIM1/region"))?,
        );

        Ok(Self {
            worlds,
            player_worlds,
        })
    }

    /// Flushes all the ready chunks into storage
    pub async fn flush_ready(&mut self) {
        for (_, w) in &mut self.worlds {
            w.chunk_provider.flush_ready().await
        }
    }

    pub async fn join_pending(&mut self) {
        for (_, w) in &mut self.worlds {
            w.chunk_provider.join_pending().await;
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
    Custom(UnlocalizedName),
}
