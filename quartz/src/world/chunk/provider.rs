use std::collections::HashMap;
use std::error::Error;
use std::io::{self, prelude::*, Cursor, Error as IoError, ErrorKind, Read, SeekFrom};
use std::fs::{self, File, OpenOptions};
use std::sync::mpsc::{self, Receiver, Sender};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use nbt::read::{read_nbt_gz_compressed, read_nbt_zlib_compressed};
use util::threadpool::{DistributionStrategy, DynamicThreadPool};
use crate::Registry;
use crate::world::{
    chunk::Chunk,
    location::{
        ChunkCoordinatePair,
        RegionCoordinatePair
    }
};

/// Minimum number of threads the pool can have
const PROVIDER_POOL_MIN_SIZE: usize = 1;
/// Maximum number of threads the pool can have
const PROVIDER_POOL_MAX_SIZE: usize = 4;
/// The maximum number of chunks that should be delegated to a worker in the pool.
const PROVIDER_POOL_LOAD_FACTOR: usize = 100;
/// The scaling between chunk coords and region coords in terms of bit shifts. The actual scale
/// factor is two to the power of this constant.
const REGION_SCALE: usize = 5;

pub struct ChunkProvider<R: Registry> {
    regions: RegionMap<R>,
    thread_pool: DynamicThreadPool<ProviderRequest, WorkerHandle<R>, Box<dyn Error>>,
    chunk_receiver: Receiver<Chunk<R>>
}

impl<R: Registry> ChunkProvider<R> {
    pub fn new<P: AsRef<Path>>(world_name: &str, root_directory: P) -> io::Result<Self> {
        // Ensure the root directory exists
        fs::create_dir_all(root_directory.as_ref())?;

        let regions = RegionMap::new(root_directory.as_ref().to_owned());
        let (chunk_sender, chunk_receiver) = mpsc::channel::<Chunk<R>>();
        let pool = DynamicThreadPool::open(
            &format!("{}/ChunkProvider", world_name),
            1, // Initial size
            WorkerHandle {
                regions: regions.clone(),
                chunk_channel: chunk_sender
            },
            DistributionStrategy::Fast,
            Self::handle_request
        );

        Ok(ChunkProvider {
            regions,
            thread_pool: pool,
            chunk_receiver
        })
    }

    pub fn request_load_full(&mut self, chunk_coords: ChunkCoordinatePair) {
        self.thread_pool.add_job(ProviderRequest::LoadFull(chunk_coords));
    }

    pub fn flush_queue(&mut self) -> Result<(), &'static str> {
        let mut map_guard = self.regions.lock()?;

        while let Ok(chunk) = self.chunk_receiver.try_recv() {
            match map_guard.loaded_region_at(chunk.chunk_coordinates() >> 5) {
                Some(region) => region.cache(chunk),
                // This should never happen
                None => drop(chunk)
            }
        }

        Ok(())
    }

    pub fn check_worker_load(&mut self) {
        self.thread_pool.resize(
            PROVIDER_POOL_LOAD_FACTOR,
            PROVIDER_POOL_MIN_SIZE,
            PROVIDER_POOL_MAX_SIZE
        );
    }

    // TODO: Add coords to error messages and change error type
    fn handle_request(request: ProviderRequest, handle: &mut WorkerHandle<R>) -> Result<(), Box<dyn Error>> {
        match request {
            ProviderRequest::LoadFull(chunk_coords) => {
                let mut map_guard = handle.regions.lock()?;
                let region = map_guard.region_at(chunk_coords >> REGION_SCALE)?;

                // The chunk data is still available, so we can just mark it as used and return early
                if region.recover_cached_chunk(chunk_coords) {
                    // Drops the guard
                    return Ok(());
                }

                let mut buffer: Vec<u8> = Vec::new();
                let saved_on_disk = region.load_chunk_nbt(chunk_coords, &mut buffer)?;

                drop(region);
                drop(map_guard);

                if saved_on_disk {
                    let nbt = match buffer[0] {
                        // GZip compression (not used in practice)
                        1 => read_nbt_gz_compressed(&mut Cursor::new(&buffer[1..]))?,
                        2 => read_nbt_zlib_compressed(&mut Cursor::new(&buffer[1..]))?,
                        _ => return Err(IoError::new(
                            ErrorKind::InvalidData,
                            format!("Encountered invalid compression scheme ({}) for chunk at {}", buffer[0], chunk_coords)
                        ).into())
                    };

                    let chunk = Chunk::from_nbt(&nbt.0);
                    match chunk {
                        Some(chunk) => handle.chunk_channel.send(chunk)?,
                        None => return Err(IoError::new(
                            ErrorKind::InvalidData,
                            format!("Encountered invalid NBT for chunk at {}", chunk_coords)
                        ).into())
                    }
                } else {
                    log::warn!("Chunk generation not supported yet.");
                }
            },
            
            ProviderRequest::Unload(chunk_coords) => {
                let region_coords = chunk_coords >> REGION_SCALE;

                let mut map_guard = handle.regions.lock()?;
                let region = match map_guard.loaded_region_at(region_coords) {
                    Some(region) => region,
                    None => return Ok(())
                };

                // Mark the cached chunk data as inactive so that the region can potentially be unloaded
                region.mark_chunk_inactive(chunk_coords);

                if region.has_loaded_chunks() {
                    return Ok(());
                }

                // We can unload the region since it has no more loaded chunks

                drop(region);
                let region = match map_guard.remove_region(region_coords) {
                    Some(region) => region,
                    None => return Ok(())
                };
                drop(map_guard);

                // TODO: write region to disk
            }
        }

        Ok(())
    }
}

struct WorkerHandle<R: Registry> {
    regions: RegionMap<R>,
    chunk_channel: Sender<Chunk<R>>
}

impl<R: Registry> Clone for WorkerHandle<R> {
    fn clone(&self) -> Self {
        WorkerHandle {
            regions: self.regions.clone(),
            chunk_channel: self.chunk_channel.clone()
        }
    }
}

pub enum ProviderRequest {
    LoadFull(ChunkCoordinatePair),
    Unload(ChunkCoordinatePair)
}

struct RegionMap<R: Registry> {
    // TODO: Consider changing to RwLock in the future
    inner: Arc<Mutex<HashMap<RegionCoordinatePair, Region<R>>>>,
    root_directory: PathBuf
}

impl<R: Registry> RegionMap<R> {
    fn new(root_directory: PathBuf) -> Self {
        RegionMap {
            inner: Arc::new(Mutex::new(HashMap::new())),
            root_directory
        }
    }

    fn lock(&self) -> Result<MapGuard<'_, R>, &'static str> {
        Ok(MapGuard {
            guard: self.inner.lock().map_err(|_| "Region map mutex poisoned")?,
            map: self
        })
    }
}

impl<R: Registry> Clone for RegionMap<R> {
    fn clone(&self) -> Self {
        RegionMap {
            inner: self.inner.clone(),
            root_directory: self.root_directory.clone()
        }
    }
}

struct MapGuard<'a, R: Registry> {
    guard: MutexGuard<'a, HashMap<RegionCoordinatePair, Region<R>>>,
    map: &'a RegionMap<R>
}

impl<'a, R: Registry> MapGuard<'a, R> {
    fn region_at(&mut self, location: RegionCoordinatePair) -> io::Result<&mut Region<R>> {
        Ok(self.guard.entry(location).or_insert(Region::new(&self.map.root_directory, location)?))
    }

    fn loaded_region_at(&mut self, location: RegionCoordinatePair) -> Option<&mut Region<R>> {
        self.guard.get_mut(&location)
    }

    fn remove_region(&mut self, location: RegionCoordinatePair) -> Option<Region<R>> {
        self.guard.remove(&location)
    }
}

struct Region<R: Registry> {
    file: File,
    chunk_offset: ChunkCoordinatePair,
    chunk_info: Box<[ChunkDataInfo<R>]>,
    loaded_count: usize
}

impl<R: Registry> Region<R> {
    fn new(root_directory: &Path, location: RegionCoordinatePair) -> io::Result<Self> {
        let file_path = root_directory.join(format!("r.{}.{}.mca", location.x, location.z));

        let mut chunk_info: Vec<ChunkDataInfo<R>> = Vec::with_capacity(1024);
        chunk_info.resize_with(1024, ChunkDataInfo::uninitialized);

        if file_path.exists() {
            let mut region = Region {
                file: OpenOptions::new().read(true).write(true).open(file_path)?,
                chunk_offset: location << REGION_SCALE,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0
            };
            region.read_file_header()?;

            Ok(region)
        } else {
            Ok(Region {
                file: File::create(file_path)?,
                chunk_offset: location << REGION_SCALE,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0
            })
        }
    }

    fn read_file_header(&mut self) -> io::Result<()> {
        if self.chunk_info.len() != 1024 {
            return Err(io::Error::new(io::ErrorKind::Other, "Region::read_file_header called with invalid or uninitialized field chunk_info."));
        }

        let mut buffer = [0_u8; 8192];
        self.file.read_exact(&mut buffer)?;

        let mut j: usize;
        let mut chunk_info: &mut ChunkDataInfo<R>;
        for i in 0..1024 {
            j = i * 4;

            // Unwrap is safe because we do a length check at the beginning of this function
            chunk_info = self.chunk_info.get_mut(i).unwrap();

            // Big endian 3-byte integer
            chunk_info.sector_offset = (buffer[j] as u32) << 16 | (buffer[j + 1] as u32) << 8 | (buffer[j + 2] as u32);
            chunk_info.sector_count = buffer[j + 3];

            // Jump to the timestamp table
            j += 4096;

            // Big endian 4-byte integer
            chunk_info.last_saved = (buffer[j] as u32) << 24 | (buffer[j + 1] as u32) << 16 | (buffer[j + 2] as u32) << 8 | (buffer[j + 3] as u32);
        }

        Ok(())
    }

    #[inline(always)]
    fn index_absolute(&self, absolute_position: ChunkCoordinatePair) -> usize {
        (absolute_position.x - self.chunk_offset.x + (absolute_position.z - self.chunk_offset.z) * 16) as usize
    }

    fn cache(&mut self, chunk: Chunk<R>) {
        if let Some(chunk_info) = self.chunk_info.get_mut(self.index_absolute(chunk.chunk_coordinates())) {
            chunk_info.cached_chunk = Some(Box::new(chunk));
            chunk_info.cache_active = true;
        }
    }

    fn recover_cached_chunk(&mut self, absolute_position: ChunkCoordinatePair) -> bool {
        let chunk = match self.chunk_info.get_mut(self.index_absolute(absolute_position)) {
            Some(chunk) => chunk,
            None => return false
        };

        if chunk.cached_chunk.is_some() {
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
    /// indicates the chunk was never saved before. If `Ok(true)` is returned, then
    fn load_chunk_nbt(&mut self, absolute_position: ChunkCoordinatePair, buffer: &mut Vec<u8>) -> io::Result<bool> {
        let chunk_info = match self.chunk_info.get_mut(self.index_absolute(absolute_position)) {
            Some(chunk_info) => chunk_info,
            None => return Err(IoError::new(ErrorKind::InvalidInput, "Attempted to load chunk outside of region"))
        };

        if chunk_info.is_uninitialized() {
            return Ok(false);
        }

        buffer.resize(((chunk_info.sector_count as usize) * 4096).max(5), 0);
        // The sector offset accounts for the tables at the beginning
        self.file.seek(SeekFrom::Start((chunk_info.sector_offset as u64) * 4096))?;
        self.file.read_exact(buffer)?;

        let length = (buffer[0] as usize) << 24 | (buffer[1] as usize) << 16 | (buffer[2] as usize) << 8 | (buffer[3] as usize);
        buffer.drain(..4);
        buffer.resize(length, 0);

        Ok(true)
    }

    fn mark_chunk_inactive(&mut self, absolute_position: ChunkCoordinatePair) {
        let chunk = match self.chunk_info.get_mut(self.index_absolute(absolute_position)) {
            Some(chunk) => chunk,
            None => return
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

struct ChunkDataInfo<R: Registry> {
    sector_offset: u32,
    sector_count: u8,
    last_saved: u32,
    cached_chunk: Option<Box<Chunk<R>>>,
    /// Whether or not the cached chunk value is actually being accessed. If this is set to false, then the region
    /// that contains this chunk data may be unloaded at any time.
    cache_active: bool
}

impl<R: Registry> ChunkDataInfo<R> {
    pub fn uninitialized() -> Self {
        ChunkDataInfo {
            sector_offset: 0,
            sector_count: 0,
            last_saved: 0,
            cached_chunk: None,
            cache_active: false
        }
    }

    pub fn is_uninitialized(&self) -> bool {
        self.last_saved == 0
    }
}