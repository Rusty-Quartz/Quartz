use crate::world::{
    chunk::{chunk::RawChunk, Chunk},
    location::{Coordinate, CoordinatePair},
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
use log::{error, warn};
use quartz_nbt::{io::NbtIoError, serde::deserialize_from_buffer};
use quartz_util::hash::NumHasher;
use std::{
    future::Future,
    io::{self, Error as IoError, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, SeekFrom},
    runtime::{Builder, Runtime},
    sync::Mutex,
};

pub struct ChunkProvider {
    pub regions: Arc<RegionHandler>,
    rt: Runtime,
}

impl ChunkProvider {
    /// Creates a chunk provider for the given root directory with the given number of threads.
    pub fn new<P: AsRef<Path>>(world_name: String, root_directory: P) -> io::Result<Self> {
        // Ensure the root directory exists
        std::fs::create_dir_all(root_directory.as_ref())?;

        let regions = Arc::new(RegionHandler::new(root_directory.as_ref().to_owned()));

        Ok(ChunkProvider {
            regions,
            rt: Builder::new_multi_thread()
                .enable_io()
                .thread_name_fn(move || {
                    static THREAD_ID: AtomicUsize = AtomicUsize::new(0);
                    format!(
                        "{}/chunk-provider#{}",
                        world_name,
                        THREAD_ID.fetch_add(1, Ordering::AcqRel)
                    )
                })
                .build()?,
        })
    }

    pub fn load_full(
        &self,
        coordinates: Coordinate,
    ) -> impl Future<Output = Result<(), NbtIoError>> + '_ {
        Self::handle_request_internal(ProviderRequest::LoadFull(coordinates), self.regions.clone())
    }

    pub fn request_load_full(&self, coordinates: Coordinate) {
        self.handle_request(ProviderRequest::LoadFull(coordinates));
    }

    fn handle_request(&self, request: ProviderRequest) {
        let regions = self.regions.clone();

        self.rt.spawn(async move {
            let result = Self::handle_request_internal(request.clone(), regions).await;

            if let Err(e) = result {
                error!("Failed to process request {:?}: {}", request, e);
            }
        });
    }

    async fn handle_request_internal(
        request: ProviderRequest,
        handler: Arc<RegionHandler>,
    ) -> Result<(), NbtIoError> {
        match request {
            ProviderRequest::LoadFull(coords) => {
                let mut region = handler.region_at(coords).await?;

                // Check if the chunk data is still available, to see if we can just mark it as
                // active and return early
                if region.recover_cached_chunk(coords) {
                    // Drops the guard
                    return Ok(());
                }

                // We skip a downgrade on `region` here because futures are lazy and the overhead
                // is minimal
                let chunk_nbt = region
                    .chunk_nbt(coords)
                    .map_err(|msg| NbtIoError::Custom(msg.to_owned().into_boxed_str()))?;

                drop(region);

                match chunk_nbt {
                    Some(chunk_nbt) => {
                        let chunk_nbt = chunk_nbt.await?;
                        Self::decode_and_cache_chunk(handler, coords, chunk_nbt).await?;
                    }
                    None => {
                        log::warn!("Chunk generation not supported yet.");
                    }
                }

                Ok(())
            }

            ProviderRequest::Unload(coords) => {
                let mut region = match handler.mut_loaded_region_at(coords) {
                    Some(region) => region,
                    None => return Ok(()),
                };

                // Mark the cached chunk data as inactive so that the region can potentially be unloaded
                region.mark_chunk_inactive(coords);

                if region.has_loaded_chunks() {
                    return Ok(());
                }

                // We can unload the region since it has no more loaded chunks

                drop(region);
                let _region = match handler.remove_region(coords) {
                    Some(region) => region,
                    None => return Ok(()),
                };

                // TODO: write region to disk
                Ok(())
            }
        }
    }

    async fn decode_and_cache_chunk(
        handler: Arc<RegionHandler>,
        coords: Coordinate,
        chunk_nbt: Vec<u8>,
    ) -> Result<(), NbtIoError> {
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
            _ =>
                return Err(IoError::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Encountered invalid compression scheme ({}) for chunk at {}",
                        chunk_nbt[0],
                        coords.as_chunk()
                    ),
                )
                .into()),
        }

        let (raw_chunk, _) = deserialize_from_buffer::<RawChunk>(&decompressed)?;
        let chunk: Chunk = raw_chunk.into();
        handler.cache_chunk(chunk);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum ProviderRequest {
    LoadFull(Coordinate),
    Unload(Coordinate),
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
    // This mutex also ensures that we do not double-load a region.
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

    async fn region_at(&self, location: Coordinate) -> io::Result<MapRefMut<'_, Region>> {
        let reg_coords = location.as_region().into();

        // Optimistically try to get a mutable reference to the region in the map
        if let Some(region) = self.regions.get_mut(&reg_coords) {
            return Ok(region);
        }

        // We use this async mutex to define a critical zone where we can have the same effect as
        // holding a reference to the underlying map while awaiting futures
        let guard = self.load_region.lock().await;

        // If we were not the first to get to the lock above, then another thread may have loaded
        // the region we want for us, so check again before executing the load.
        if let Some(region) = self.regions.get_mut(&reg_coords) {
            return Ok(region);
        }

        let start = Instant::now();
        let region = Region::new(&self.root_directory, location).await?;
        let region_ref = self.regions.entry(reg_coords).or_insert(region);
        let elapsed = start.elapsed();
        log::info!("Region load time: {:?}", elapsed);

        drop(guard);

        Ok(region_ref)
    }

    #[inline]
    pub fn loaded_region_at(&self, location: Coordinate) -> Option<MapRef<'_, Region>> {
        self.regions.get(&location.as_region().into())
    }

    #[inline]
    pub fn mut_loaded_region_at(&self, location: Coordinate) -> Option<MapRefMut<'_, Region>> {
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
    pub fn mut_loaded_chunk_at(&self, location: Coordinate) -> Option<MapRefMut<'_, Chunk>> {
        self.chunks.get_mut(&location.as_chunk().into())
    }

    fn cache_chunk(&self, chunk: Chunk) {
        let coords = chunk.coordinates();

        let mut region = match self.mut_loaded_region_at(coords) {
            Some(region) => region,
            None => {
                error!(
                    "Failed to cache chunk at {:?}: associated region not loaded.",
                    coords
                );
                return;
            }
        };

        match region.mut_chunk_info_at(coords) {
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

        let mut chunk_info: Vec<ChunkMetadata> = Vec::with_capacity(1024);
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

        let mut buffer = vec![0; 8192].into_boxed_slice();
        self.file.lock().await.read_exact(&mut *buffer).await?;

        let mut j: usize;
        let mut chunk_info: &mut ChunkMetadata;
        for i in 0 .. 1024 {
            j = i * 4;

            // Unwrap is safe because we do a length check at the beginning of this function
            chunk_info = self.chunk_info.get_mut(i).unwrap();

            // Big endian 3-byte integer
            chunk_info.sector_offset =
                (buffer[j] as u32) << 16 | (buffer[j + 1] as u32) << 8 | (buffer[j + 2] as u32);
            chunk_info.sector_count = buffer[j + 3];

            // Jump to the timestamp table
            j += 4096;

            // Big endian 4-byte integer
            chunk_info.last_saved = (buffer[j] as u32) << 24
                | (buffer[j + 1] as u32) << 16
                | (buffer[j + 2] as u32) << 8
                | (buffer[j + 3] as u32);
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

    fn mut_chunk_info_at(&mut self, absolute_position: Coordinate) -> Option<&mut ChunkMetadata> {
        self.chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
    }

    fn recover_cached_chunk(&mut self, absolute_position: Coordinate) -> bool {
        let chunk = match self.mut_chunk_info_at(absolute_position) {
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

    /// Attempts to load the chunk from disk, returning the `Ok` variant when no IO errors occur. The boolean
    /// returned indicates whether or not the chunk was actually on disk, if false is returned then that
    /// indicates the chunk was never saved before. If `Ok(true)` is returned, then the chunk was saved on disk
    /// before and has been loaded into the given buffer.
    fn chunk_nbt(
        &self,
        absolute_position: Coordinate,
    ) -> Result<Option<impl Future<Output = Result<Vec<u8>, NbtIoError>>>, &'static str> {
        let file = self.file.clone();

        let chunk_info = match self.chunk_info_at(absolute_position) {
            Some(chunk_info) => chunk_info,
            None => return Err("Attempted to load chunk outside of region"),
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

            let mut buf: Vec<u8> = Vec::with_capacity(length);
            unsafe {
                buf.set_len(length);
            }

            file_lock.read_exact(&mut buf).await?;
            drop(file_lock);

            Ok(buf)
        }))
    }

    fn mark_chunk_inactive(&mut self, absolute_position: Coordinate) {
        let chunk = match self.mut_chunk_info_at(absolute_position) {
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
