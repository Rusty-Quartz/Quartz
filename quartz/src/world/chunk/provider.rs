use crate::{
    network::{
        packet_data::SectionAndLightData,
        AsyncWriteHandle,
        ClientBoundPacket,
        WrappedClientBoundPacket,
    },
    world::chunk::{chunk::RawChunk, Chunk, ChunkDecodeError, RawClientChunk},
};
use byteorder::{BigEndian, ByteOrder};
use dashmap::{
    mapref::{
        multiple::RefMulti,
        one::{Ref, RefMut},
    },
    DashMap,
};
use flate2::write::{GzDecoder, ZlibDecoder};
use futures_util::{poll, stream::FuturesUnordered, StreamExt};
use log::{error, warn};
use qdat::world::location::{Coordinate, CoordinatePair};
use quartz_nbt::serde::deserialize_from_buffer;
use quartz_util::hash::NumHasher;
use serde::Deserialize;
use std::{
    convert,
    fmt::{self, Display, Formatter},
    future::Future,
    io::{self, Error as IoError, Write},
    path::{Path, PathBuf},
    sync::Arc,
    task::Poll,
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
    runtime::Runtime,
    sync::Mutex,
    task::{JoinError, JoinHandle},
};

pub struct ChunkProvider {
    pub store: Arc<RegionHandler>,
    rt: Arc<Runtime>,
    pending: FuturesUnordered<JoinHandle<Result<ProviderResponse, ProviderError>>>,
}

impl ChunkProvider {
    /// Creates a chunk provider for the given root directory with the given number of threads.
    pub fn new<P: AsRef<Path>>(
        rt: Arc<Runtime>,
        root_directory: P
    ) -> io::Result<Self> {
        let root_directory = root_directory.as_ref();

        // Ensure the root directory exists
        std::fs::create_dir_all(root_directory)?;

        let store = Arc::new(RegionHandler::new(root_directory.to_owned()));
        let pending = FuturesUnordered::new();

        Ok(ChunkProvider { store, rt, pending })
    }

    pub fn request(&self, request: ProviderRequest) {
        let store = self.store.clone();
        let fut = self.rt.spawn(Self::handle_request_internal(request, store));
        self.pending.push(fut);
    }

    pub async fn flush_ready(&mut self) {
        while let Poll::Ready(Some(task_result)) = poll!(self.pending.next()) {
            self.handle_task_result(task_result);
        }
    }

    pub async fn join_pending(&mut self) {
        while let Some(task_result) = self.pending.next().await {
            self.handle_task_result(task_result);
        }
    }

    fn handle_task_result(
        &self,
        task_result: Result<Result<ProviderResponse, ProviderError>, JoinError>,
    ) {
        let result = match task_result {
            Ok(result) => result,
            Err(error) => {
                // We propagate panics up so we can handle them correctly
                panic!("Internal error in chunk provider: {}", error);
            }
        };

        let response = match result {
            Ok(response) => response,
            Err(error) => {
                // This error is recoverable
                error!("Error in chunk provider: {}", error);
                return;
            }
        };

        match response {
            ProviderResponse::LoadedChunk(chunk) =>
                if let Some(chunk) = chunk {
                    self.store.cache_chunk(chunk);
                },
            ProviderResponse::UnloadedChunk => {
                // TODO: handle anything here if necessary
            }
            ProviderResponse::Ok => {
                // yay!
            }
        }
    }

    async fn handle_request_internal(
        request: ProviderRequest,
        store: Arc<RegionHandler>,
    ) -> Result<ProviderResponse, ProviderError> {
        match request {
            ProviderRequest::LoadFull(coords) => Self::handle_load_full(coords, store)
                .await
                .map(ProviderResponse::LoadedChunk)
                .map_err(|error| ProviderError::new(request, error)),

            ProviderRequest::MinLoadSend { coords, ref handle } =>
                Self::handle_load_send(coords, handle, store)
                    .await
                    .map(|_| ProviderResponse::Ok)
                    .map_err(|error| ProviderError::new(request, error)),

            ProviderRequest::Unload(coords) => {
                Self::handle_unload(coords, store).await;
                Ok(ProviderResponse::UnloadedChunk)
            }
        }
    }

    async fn handle_load_full(
        coords: Coordinate,
        store: Arc<RegionHandler>,
    ) -> Result<Option<Chunk>, ChunkDecodeError> {
        let mut region = store.region_at_mut(coords).await?;

        // Check if the chunk data is still available, to see if we can just mark it as
        // active and return early
        if region.recover_cached_chunk(coords) {
            // Drops the guard
            return Ok(None);
        }

        // We skip a downgrade on `region` here because futures are lazy and the overhead
        // is minimal
        let chunk_nbt = region.chunk_nbt(coords)?;

        drop(region);

        match chunk_nbt {
            Some(chunk_nbt) => {
                let chunk_nbt = chunk_nbt.await?;
                return Self::decode_chunk::<RawChunk, _, _>(chunk_nbt, Chunk::from)
                    .await
                    .map(Some);
            }
            None => {
                log::warn!("Chunk generation not supported yet.");
            }
        }

        Ok(None)
    }

    async fn handle_load_send(
        coords: Coordinate,
        handle: &AsyncWriteHandle,
        store: Arc<RegionHandler>,
    ) -> Result<(), ChunkDecodeError> {
        // This is very similar to load full except it drops the chunk once the data is sent, and
        // does a minimal load

        let chunk_coords: CoordinatePair = coords.as_chunk().into();
        let chunk_x = chunk_coords.x;
        let chunk_z = chunk_coords.z;

        // Two packets: ChunkData and UpdateLight
        let mut packets = Vec::with_capacity(2);

        // If it's already loaded we can just send it
        if let Some(chunk) = store.loaded_chunk_at(coords) {
            let (primary_bit_mask, section_data) = chunk.gen_client_section_data();

            packets.push(WrappedClientBoundPacket::Singleton(
                ClientBoundPacket::ChunkData {
                    chunk_x,
                    chunk_z,
                    primary_bit_mask,
                    heightmaps: chunk.get_heightmaps(),
                    biomes: Box::from(chunk.biomes()),
                    // TODO: send block entities for chunk when we support them
                    block_entities: vec![].into_boxed_slice(),
                    data: section_data,
                },
            ));

            let (sky_light_mask, empty_sky_light_mask, sky_light_arrays) = chunk.gen_sky_lights();
            let (block_light_mask, empty_block_light_mask, block_light_arrays) =
                chunk.gen_block_lights();

            packets.push(WrappedClientBoundPacket::Singleton(
                ClientBoundPacket::UpdateLight {
                    chunk_x,
                    chunk_z,
                    trust_edges: true,
                    sky_light_mask,
                    block_light_mask,
                    empty_sky_light_mask,
                    empty_block_light_mask,
                    sky_light_arrays,
                    block_light_arrays,
                },
            ));

            drop(chunk);
            handle.send_all(packets);
            return Ok(());
        }

        let region = store.region_at(coords).await?;

        // We don't try to recover the cached chunk since the user should've requested a full load
        // send if that was desired

        let chunk_nbt = region.chunk_nbt(coords)?;

        drop(region);

        match chunk_nbt {
            Some(chunk_nbt) => {
                let chunk_nbt = chunk_nbt.await?;
                let chunk =
                    Self::decode_chunk(chunk_nbt, |raw_chunk: RawClientChunk| raw_chunk.level)
                        .await?;

                let SectionAndLightData {
                    primary_bit_mask,
                    sections,
                    block_light_mask,
                    sky_light_mask,
                    empty_block_light_mask,
                    empty_sky_light_mask,
                    block_light,
                    sky_light,
                } = chunk.sections.into_packet_data();

                let heightmaps = chunk.heightmaps;
                let biomes = chunk.biomes;
                // TODO: send block entities for chunk when we support them
                let block_entities = Vec::new().into_boxed_slice();

                packets.push(WrappedClientBoundPacket::Singleton(
                    ClientBoundPacket::ChunkData {
                        chunk_x,
                        chunk_z,
                        primary_bit_mask,
                        heightmaps,
                        biomes,
                        block_entities,
                        data: sections,
                    },
                ));

                packets.push(WrappedClientBoundPacket::Singleton(
                    ClientBoundPacket::UpdateLight {
                        chunk_x,
                        chunk_z,
                        trust_edges: true,
                        sky_light_mask,
                        block_light_mask,
                        empty_sky_light_mask,
                        empty_block_light_mask,
                        sky_light_arrays: sky_light,
                        block_light_arrays: block_light,
                    },
                ));
            }
            None => {
                log::warn!("Chunk generation not supported yet.");
            }
        }

        handle.send_all(packets);
        Ok(())
    }

    async fn decode_chunk<D, R, F>(chunk_nbt: Vec<u8>, f: F) -> Result<R, ChunkDecodeError>
    where
        for<'a> D: Deserialize<'a>,
        F: FnOnce(D) -> R,
    {
        let mut decompressed = Vec::new();

        match chunk_nbt[0] {
            2 => {
                let mut decoder = ZlibDecoder::new(decompressed);
                decoder.write_all(&chunk_nbt[1 ..])?;
                decompressed = decoder.finish()?;
            }
            // GZip compression (not used in practice)
            1 => {
                let mut decoder = GzDecoder::new(decompressed);
                decoder.write_all(&chunk_nbt[1 ..])?;
                decompressed = decoder.finish()?;
            }
            _ => return Err(ChunkDecodeError::UnknownCompression(chunk_nbt[0])),
        }

        let (raw, _) = deserialize_from_buffer::<D>(&decompressed)?;
        Ok(f(raw))
    }

    async fn handle_unload(coords: Coordinate, store: Arc<RegionHandler>) {
        let mut region = match store.loaded_region_at_mut(coords) {
            Some(region) => region,
            None => return,
        };

        // Mark the cached chunk data as inactive so that the region can potentially be unloaded
        region.mark_chunk_inactive(coords);

        if region.has_loaded_chunks() {
            return;
        }

        // We can unload the region since it has no more loaded chunks

        drop(region);
        let _region = match store.remove_region(coords) {
            Some(region) => region,
            None => return,
        };

        // TODO: write region to disk
    }
}

#[derive(Clone, Debug)]
pub enum ProviderRequest {
    LoadFull(Coordinate),
    MinLoadSend {
        coords: Coordinate,
        handle: AsyncWriteHandle,
    },
    Unload(Coordinate),
}

enum ProviderResponse {
    LoadedChunk(Option<Chunk>),
    // TODO: add more semantic information
    UnloadedChunk,
    Ok,
}

#[derive(Debug)]
pub struct ProviderError {
    request: ProviderRequest,
    error: ProviderErrorType,
}

impl ProviderError {
    fn new(request: ProviderRequest, error: impl Into<ProviderErrorType>) -> Self {
        Self {
            request,
            error: error.into(),
        }
    }
}

impl Display for ProviderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to handle provider request {:?}: ", self.request)?;

        match &self.error {
            ProviderErrorType::ChunkDecode(error) => Display::fmt(error, f),
        }
    }
}

#[derive(Debug)]
pub enum ProviderErrorType {
    ChunkDecode(ChunkDecodeError),
}

impl From<ChunkDecodeError> for ProviderErrorType {
    fn from(error: ChunkDecodeError) -> Self {
        Self::ChunkDecode(error)
    }
}

pub type Map<T> = DashMap<CoordinatePair, T, NumHasher>;
pub type MapRef<'a, T> = Ref<'a, CoordinatePair, T, NumHasher>;
pub type MapRefMut<'a, T> = RefMut<'a, CoordinatePair, T, NumHasher>;
pub type MapRefMulti<'a, T> = RefMulti<'a, CoordinatePair, T, NumHasher>;

pub struct RegionHandler {
    regions: Map<Region>,
    chunks: Map<Chunk>,
    root_directory: PathBuf,
    // Dashmap will deadlock if a ref is held across a `.await`, so we use this async mutex to
    // gain exclusive access for the specific operation of inserting a region into the region map.
    // Correct usage of this mutex also ensures that we do not double-load a region.
    load_region: Mutex<()>,
}

impl RegionHandler {
    fn new(root_directory: PathBuf) -> Self {
        RegionHandler {
            regions: Map::with_hasher(NumHasher),
            chunks: Map::with_hasher(NumHasher),
            root_directory,
            load_region: Mutex::new(()),
        }
    }

    async fn region_at(&self, location: Coordinate) -> io::Result<MapRef<'_, Region>> {
        self.region_at_internal(
            location,
            |regions, coords| regions.get(coords),
            |ref_mut| ref_mut.downgrade(),
        )
        .await
    }

    async fn region_at_mut(&self, location: Coordinate) -> io::Result<MapRefMut<'_, Region>> {
        self.region_at_internal(
            location,
            |regions, coords| regions.get_mut(coords),
            convert::identity,
        )
        .await
    }

    async fn region_at_internal<'a, F, M, R>(
        &'a self,
        location: Coordinate,
        mut try_acquire_fast: F,
        map_acquired_slow: M,
    ) -> io::Result<R>
    where
        F: FnMut(&'a Map<Region>, &CoordinatePair) -> Option<R>,
        M: FnOnce(MapRefMut<'a, Region>) -> R,
    {
        let reg_coords = location.as_region().into();

        // Optimistically try to get a mutable reference to the region in the map. This will only
        // fail if the region has not yet been loaded, which could happen in one of the following
        // scenarios:
        //    1. A player joins the game in an unloaded area
        //    2. A player comes within view distance of a new region
        //    3. A player teleports to another region
        // 1 is fairly infrequent, 2 is also infrequent except in the case where a player is using
        // and elytra, and 3 is also infrequent on vanilla servers.
        if let Some(region) = try_acquire_fast(&self.regions, &reg_coords) {
            return Ok(region);
        }

        // We use this async mutex to define a critical zone where we can have the same effect as
        // holding a reference to the underlying map while awaiting futures
        let guard = self.load_region.lock().await;

        // If we were not the first to get to the lock above, then another thread may have loaded
        // the region we want for us, so check again before executing the load.
        if let Some(region) = try_acquire_fast(&self.regions, &reg_coords) {
            return Ok(region);
        }

        // Use a let binding to avoid holding a reference across and await
        let region = Region::new(&self.root_directory, location).await?;

        // Insert the region
        let region_ref = self.regions.entry(reg_coords).or_insert(region);

        // Drop the guard after we insert the region into the map to ensure we don't cause a
        // double-load
        drop(guard);

        Ok(map_acquired_slow(region_ref))
    }

    #[inline]
    pub fn loaded_region_at(&self, location: Coordinate) -> Option<MapRef<'_, Region>> {
        self.regions.get(&location.as_region().into())
    }

    #[inline]
    pub fn loaded_region_at_mut(&self, location: Coordinate) -> Option<MapRefMut<'_, Region>> {
        self.regions.get_mut(&location.as_region().into())
    }

    #[inline]
    pub fn remove_region(&self, location: Coordinate) -> Option<Region> {
        self.regions
            .remove(&location.as_region().into())
            .map(|(_, region)| region)
    }

    #[inline]
    pub fn loaded_chunks(&self) -> impl Iterator<Item = MapRefMulti<'_, Chunk>> {
        self.chunks.iter()
    }

    #[inline]
    pub fn loaded_chunk_at(&self, location: Coordinate) -> Option<MapRef<'_, Chunk>> {
        self.chunks.get(&location.as_chunk().into())
    }

    #[inline]
    pub fn loaded_chunk_at_mut(&self, location: Coordinate) -> Option<MapRefMut<'_, Chunk>> {
        self.chunks.get_mut(&location.as_chunk().into())
    }

    fn cache_chunk(&self, chunk: Chunk) {
        let coords = chunk.coordinates();

        let mut region = match self.loaded_region_at_mut(coords) {
            Some(region) => region,
            None => {
                error!(
                    "Failed to cache chunk at {:?}: associated region not loaded.",
                    coords
                );
                return;
            }
        };

        match region.chunk_info_at_mut(coords) {
            Some(chunk_info) => {
                chunk_info.cache_inhabited = true;
                chunk_info.cache_active = true;
            }
            None => {
                // We explicitly panic here because this is a serious bug
                panic!(
                    "Failed to cache chunk at {:?}: region and chunk coordinates not synchronized",
                    coords
                );
            }
        }

        drop(region);

        let entry = self.chunks.insert(coords.as_chunk().into(), chunk);
        if entry.is_some() {
            warn!("Overwrote chunk at {:?} while caching new chunk", coords)
        }
    }
}

pub struct Region {
    file: Arc<Mutex<File>>,
    chunk_offset: CoordinatePair,
    loaded_count: usize,
    chunk_info: Box<[ChunkMetadata]>,
}

impl Region {
    async fn new(root_directory: &Path, location: Coordinate) -> io::Result<Self> {
        let chunk_offset: CoordinatePair = location.as_region().as_chunk().into();
        let region_offset: CoordinatePair = location.as_region().into();

        let file_path =
            root_directory.join(format!("r.{}.{}.mca", region_offset.x, region_offset.z));

        let mut chunk_info = Vec::new();
        chunk_info.resize_with(1024, ChunkMetadata::uninitialized);

        if file_path.exists() {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(file_path)
                .await?;
            let mut region = Region {
                file: Arc::new(Mutex::new(file)),
                chunk_offset,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0,
            };
            region.read_file_header().await?;

            Ok(region)
        } else {
            Ok(Region {
                file: Arc::new(Mutex::new(File::create(file_path).await?)),
                chunk_offset,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0,
            })
        }
    }

    async fn read_file_header(&mut self) -> io::Result<()> {
        assert_eq!(
            self.chunk_info.len(),
            1024,
            "Region::read_file_header called with invalid or uninitialized field chunk_info."
        );

        let mut buffer = vec![0; 8192];
        self.file.lock().await.read_exact(&mut *buffer).await?;

        for mut i in 0 .. 1024 {
            // Unwrap is safe because we do a length check at the beginning of this function
            let chunk_info = self.chunk_info.get_mut(i).unwrap();

            // Align with the sector metadata table
            i *= 4;

            chunk_info.sector_offset = BigEndian::read_u24(&buffer[i .. i + 3]);
            chunk_info.sector_count = buffer[i + 3];

            // Jump to the timestamp table
            i += 4096;

            // Big endian 4-byte integer
            chunk_info.last_saved = BigEndian::read_u32(&buffer[i .. i + 4]);
        }

        Ok(())
    }

    #[inline]
    fn index_absolute(&self, absolute_position: CoordinatePair) -> usize {
        (absolute_position.x - self.chunk_offset.x
            + (absolute_position.z - self.chunk_offset.z) * 32) as usize
    }

    fn chunk_info_at(&self, absolute_position: Coordinate) -> Option<&ChunkMetadata> {
        self.chunk_info
            .get(self.index_absolute(absolute_position.as_chunk().into()))
    }

    fn chunk_info_at_mut(&mut self, absolute_position: Coordinate) -> Option<&mut ChunkMetadata> {
        self.chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
    }

    fn recover_cached_chunk(&mut self, absolute_position: Coordinate) -> bool {
        let chunk = match self.chunk_info_at_mut(absolute_position) {
            Some(chunk) => chunk,
            None => return false,
        };

        if chunk.cache_inhabited {
            if !chunk.cache_active {
                chunk.cache_active = true;
                self.loaded_count += 1;
            }
            true
        } else {
            false
        }
    }

    fn chunk_nbt(
        &self,
        absolute_position: Coordinate,
    ) -> Result<Option<impl Future<Output = Result<Vec<u8>, IoError>>>, ChunkDecodeError> {
        let file = self.file.clone();

        let chunk_info = match self.chunk_info_at(absolute_position) {
            Some(chunk_info) => chunk_info,
            None =>
                return Err(ChunkDecodeError::ChunkRegionDesync(
                    absolute_position.as_chunk(),
                )),
        };

        if chunk_info.is_uninitialized() {
            return Ok(None);
        }

        // The sector offset accounts for the tables at the beginning
        let seek_offset = (chunk_info.sector_offset as u64) * 4096;

        Ok(Some(async move {
            let mut file_lock = file.lock().await;
            file_lock.seek(SeekFrom::Start(seek_offset)).await?;

            // Read the length
            let mut buf = [0u8; 4];
            file_lock.read_exact(&mut buf).await?;
            let length = BigEndian::read_u32(&buf) as usize;

            // Safety:
            // This is not UB because while the memory is uninitalized, u8s are still valid
            // We are only ever reading from the file which does not depend on the memory in the Vec
            // This allows us to avoid UB as the read will initialize the memory with valid data
            #[allow(clippy::uninit_vec)]
            let buf = {
                let mut buf: Vec<u8> = Vec::with_capacity(length);
                unsafe {
                    buf.set_len(length);
                }

                file_lock.read_exact(&mut buf).await?;
                buf
            };
            drop(file_lock);

            Ok(buf)
        }))
    }

    fn mark_chunk_inactive(&mut self, absolute_position: Coordinate) {
        let chunk = match self.chunk_info_at_mut(absolute_position) {
            Some(chunk) => chunk,
            None => return,
        };

        if chunk.cache_active {
            chunk.cache_active = false;
            self.loaded_count -= 1;
        }
    }

    fn has_loaded_chunks(&self) -> bool {
        self.loaded_count > 0
    }
}

#[derive(Clone, Copy)]
struct ChunkMetadata {
    sector_offset: u32,
    last_saved: u32,
    sector_count: u8,
    /// Whether or not the corresponding chunk is cached.
    cache_inhabited: bool,
    /// Whether or not the cached chunk value is actually being accessed. If this is set to false, then the region
    /// that contains this chunk data may be unloaded at any time if no other chunks are loaded.
    cache_active: bool,
}

impl ChunkMetadata {
    pub fn uninitialized() -> Self {
        ChunkMetadata {
            sector_offset: 0,
            sector_count: 0,
            last_saved: 0,
            cache_inhabited: false,
            cache_active: false,
        }
    }

    pub fn is_uninitialized(&self) -> bool {
        self.last_saved == 0
    }
}
