use crate::{
    world::{
        chunk::Chunk,
        location::{ChunkCoordinatePair, RegionCoordinatePair},
    },
    Registry,
};
use futures_lite::{
    future,
    io::{AsyncReadExt, AsyncSeekExt},
};
use log::error;
use nbt::read::{read_nbt_gz_compressed, read_nbt_zlib_compressed};
use smol::{
    channel::{self, Receiver, Sender},
    fs::{File, OpenOptions},
    io::{self, Error as IoError, ErrorKind, SeekFrom},
    lock::{Mutex, MutexGuard},
    Executor,
};
use std::{
    collections::HashMap,
    io::Cursor as StdCursor,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

/// The scaling between chunk coords and region coords in terms of bit shifts. The actual scale
/// factor is two to the power of this constant.
const REGION_SCALE: usize = 5;

pub struct ChunkProvider<R: Registry> {
    regions: RegionMap<R>,
    chunk_sender: Sender<Chunk<R>>,
    chunk_receiver: Receiver<Chunk<R>>,
    executor: Arc<Executor<'static>>,
    _shutdown_signal: Sender<()>,
}

impl<R: Registry> ChunkProvider<R> {
    /// Creates a chunk provider for the given root directory with the given number of threads.
    pub fn new<P: AsRef<Path>>(
        world_name: &str,
        root_directory: P,
        thread_count: usize,
    ) -> io::Result<Self>
    {
        // Ensure the root directory exists
        std::fs::create_dir_all(root_directory.as_ref())?;

        let regions = RegionMap::new(root_directory.as_ref().to_owned());
        let (chunk_sender, chunk_receiver) = channel::unbounded::<Chunk<R>>();
        let (shutdown_signal, shutdown) = channel::unbounded::<()>();

        let executor = Arc::new(Executor::new());
        for i in 1 ..= usize::max(thread_count, 1) {
            let shutdown_clone = shutdown.clone();
            let executor_clone = executor.clone();

            thread::Builder::new()
                .name(format!("{}/ChunkProvider-{}", world_name, i))
                .spawn(move || future::block_on(executor_clone.run(shutdown_clone.recv())))?;
        }

        Ok(ChunkProvider {
            regions,
            chunk_sender,
            chunk_receiver,
            executor,
            _shutdown_signal: shutdown_signal,
        })
    }

    pub async fn lock_regions(&self) -> RegionMapGuard<'_, R> {
        self.regions.lock().await
    }

    pub fn request_load_full(&mut self, chunk_coords: ChunkCoordinatePair) {
        self.handle_request(ProviderRequest::LoadFull(chunk_coords));
    }

    pub async fn flush_queue(&mut self) {
        let mut map_guard = self.regions.lock().await;

        while let Ok(chunk) = self.chunk_receiver.try_recv() {
            match map_guard.loaded_region_at(chunk.chunk_coordinates() >> 5) {
                Some(region) => region.cache(chunk),
                // This should never happen
                None => drop(chunk),
            }
        }
    }

    fn handle_request(&self, request: ProviderRequest) {
        let regions = self.regions.clone();
        let chunk_sender = self.chunk_sender.clone();

        self.executor
            .spawn(async move {
                let result =
                    Self::handle_request_internal(request.clone(), regions, chunk_sender).await;

                if let Err(e) = result {
                    error!("Failed to process request {:?}: {}", request, e);
                }
            })
            .detach();
    }

    async fn handle_request_internal(
        request: ProviderRequest,
        regions: RegionMap<R>,
        chunk_sender: Sender<Chunk<R>>,
    ) -> io::Result<()>
    {
        match request {
            ProviderRequest::LoadFull(chunk_coords) => {
                let mut map_guard = regions.lock().await;
                let region = map_guard.region_at(chunk_coords >> REGION_SCALE).await?;

                // The chunk data is still available, so we can just mark it as used and return early
                if region.recover_cached_chunk(chunk_coords) {
                    // Drops the guard
                    return Ok(());
                }

                let mut buffer: Vec<u8> = Vec::new();
                let saved_on_disk = region.load_chunk_nbt(chunk_coords, &mut buffer).await?;

                drop(region);
                drop(map_guard);

                if saved_on_disk {
                    let nbt = match buffer[0] {
                        // GZip compression (not used in practice)
                        1 => read_nbt_gz_compressed(&mut StdCursor::new(&buffer[1 ..]))?,
                        2 => read_nbt_zlib_compressed(&mut StdCursor::new(&buffer[1 ..]))?,
                        _ =>
                            return Err(IoError::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "Encountered invalid compression scheme ({}) for chunk at {}",
                                    buffer[0], chunk_coords
                                ),
                            )),
                    };

                    let chunk = Chunk::from_nbt(&nbt.0);
                    match chunk {
                        Ok(chunk) => chunk_sender.send(chunk).await.map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "Failed to send chunk between threads",
                            )
                        })?,
                        Err(e) =>
                            return Err(IoError::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "Encountered invalid NBT for chunk at {}: {}",
                                    chunk_coords, e
                                ),
                            )
                            .into()),
                    }
                } else {
                    log::warn!("Chunk generation not supported yet.");
                }
            }

            ProviderRequest::Unload(chunk_coords) => {
                let region_coords = chunk_coords >> REGION_SCALE;

                let mut map_guard = regions.lock().await;
                let region = match map_guard.loaded_region_at(region_coords) {
                    Some(region) => region,
                    None => return Ok(()),
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
                    None => return Ok(()),
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
    chunk_channel: Sender<Chunk<R>>,
}

impl<R: Registry> Clone for WorkerHandle<R> {
    fn clone(&self) -> Self {
        WorkerHandle {
            regions: self.regions.clone(),
            chunk_channel: self.chunk_channel.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProviderRequest {
    LoadFull(ChunkCoordinatePair),
    Unload(ChunkCoordinatePair),
}

pub struct RegionMap<R: Registry> {
    // TODO: Consider changing to RwLock in the future
    inner: Arc<Mutex<HashMap<RegionCoordinatePair, Region<R>>>>,
    root_directory: PathBuf,
}

impl<R: Registry> RegionMap<R> {
    fn new(root_directory: PathBuf) -> Self {
        RegionMap {
            inner: Arc::new(Mutex::new(HashMap::new())),
            root_directory,
        }
    }

    async fn lock(&self) -> RegionMapGuard<'_, R> {
        RegionMapGuard {
            guard: self.inner.lock().await,
            map: self,
        }
    }
}

impl<R: Registry> Clone for RegionMap<R> {
    fn clone(&self) -> Self {
        RegionMap {
            inner: self.inner.clone(),
            root_directory: self.root_directory.clone(),
        }
    }
}

pub struct RegionMapGuard<'a, R: Registry> {
    guard: MutexGuard<'a, HashMap<RegionCoordinatePair, Region<R>>>,
    map: &'a RegionMap<R>,
}

impl<'a, R: Registry> RegionMapGuard<'a, R> {
    pub fn loaded_chunk_at(&mut self, location: ChunkCoordinatePair) -> Option<&mut Chunk<R>> {
        self.guard.get_mut(&(location >> 5)).map(|region| region.chunk_at(location)).flatten()
    }

    async fn region_at(&mut self, location: RegionCoordinatePair) -> io::Result<&mut Region<R>> {
        Ok(self
            .guard
            .entry(location)
            .or_insert(Region::new(&self.map.root_directory, location).await?))
    }

    pub fn loaded_region_at(&mut self, location: RegionCoordinatePair) -> Option<&mut Region<R>> {
        self.guard.get_mut(&location)
    }

    fn remove_region(&mut self, location: RegionCoordinatePair) -> Option<Region<R>> {
        self.guard.remove(&location)
    }
}

pub struct Region<R: Registry> {
    file: File,
    chunk_offset: ChunkCoordinatePair,
    chunk_info: Box<[ChunkDataInfo<R>]>,
    loaded_count: usize,
}

impl<R: Registry> Region<R> {
    async fn new(root_directory: &Path, location: RegionCoordinatePair) -> io::Result<Self> {
        let file_path = root_directory.join(format!("r.{}.{}.mca", location.x, location.z));

        let mut chunk_info: Vec<ChunkDataInfo<R>> = Vec::with_capacity(1024);
        chunk_info.resize_with(1024, ChunkDataInfo::uninitialized);

        if file_path.exists() {
            let mut region = Region {
                file: OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(file_path)
                    .await?,
                chunk_offset: location << REGION_SCALE,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0,
            };
            region.read_file_header().await?;

            Ok(region)
        } else {
            Ok(Region {
                file: File::create(file_path).await?,
                chunk_offset: location << REGION_SCALE,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0,
            })
        }
    }

    async fn read_file_header(&mut self) -> io::Result<()> {
        if self.chunk_info.len() != 1024 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Region::read_file_header called with invalid or uninitialized field chunk_info.",
            ));
        }

        let mut buffer = [0_u8; 8192];
        self.file.read_exact(&mut buffer).await?;

        let mut j: usize;
        let mut chunk_info: &mut ChunkDataInfo<R>;
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

    #[inline(always)]
    fn index_absolute(&self, absolute_position: ChunkCoordinatePair) -> usize {
        (absolute_position.x - self.chunk_offset.x
            + (absolute_position.z - self.chunk_offset.z) * 16) as usize
    }

    pub fn chunk_at(&mut self, absolute_position: ChunkCoordinatePair) -> Option<&mut Chunk<R>> {
        let chunk = self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position))?;

        if chunk.cached_chunk.is_some() {
            if !chunk.cache_active {
                chunk.cache_active = true;
                self.loaded_count += 1;
            }
            chunk.cached_chunk.as_mut()
        } else {
            None
        }
    }

    fn cache(&mut self, chunk: Chunk<R>) {
        if let Some(chunk_info) = self
            .chunk_info
            .get_mut(self.index_absolute(chunk.chunk_coordinates()))
        {
            chunk_info.cached_chunk = Some(chunk);
            chunk_info.cache_active = true;
        }
    }

    fn recover_cached_chunk(&mut self, absolute_position: ChunkCoordinatePair) -> bool {
        let chunk = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position))
        {
            Some(chunk) => chunk,
            None => return false,
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
    /// indicates the chunk was never saved before. If `Ok(true)` is returned, then the chunk was saved on disk
    /// before and has been loaded into the given buffer.
    async fn load_chunk_nbt(
        &mut self,
        absolute_position: ChunkCoordinatePair,
        buffer: &mut Vec<u8>,
    ) -> io::Result<bool>
    {
        let chunk_info = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position))
        {
            Some(chunk_info) => chunk_info,
            None =>
                return Err(IoError::new(
                    ErrorKind::InvalidInput,
                    "Attempted to load chunk outside of region",
                )),
        };

        if chunk_info.is_uninitialized() {
            return Ok(false);
        }

        buffer.resize(((chunk_info.sector_count as usize) * 4096).max(5), 0);
        // The sector offset accounts for the tables at the beginning
        self.file
            .seek(SeekFrom::Start((chunk_info.sector_offset as u64) * 4096))
            .await?;
        self.file.read_exact(buffer).await?;

        let length = (buffer[0] as usize) << 24
            | (buffer[1] as usize) << 16
            | (buffer[2] as usize) << 8
            | (buffer[3] as usize);
        buffer.drain(.. 4);
        buffer.resize(length, 0);

        Ok(true)
    }

    fn mark_chunk_inactive(&mut self, absolute_position: ChunkCoordinatePair) {
        let chunk = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position))
        {
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

// TODO: Consider boxing
struct ChunkDataInfo<R: Registry> {
    sector_offset: u32,
    sector_count: u8,
    last_saved: u32,
    cached_chunk: Option<Chunk<R>>,
    /// Whether or not the cached chunk value is actually being accessed. If this is set to false, then the region
    /// that contains this chunk data may be unloaded at any time if no other chunks are loaded.
    cache_active: bool,
}

impl<R: Registry> ChunkDataInfo<R> {
    pub fn uninitialized() -> Self {
        ChunkDataInfo {
            sector_offset: 0,
            sector_count: 0,
            last_saved: 0,
            cached_chunk: None,
            cache_active: false,
        }
    }

    pub fn is_uninitialized(&self) -> bool {
        self.last_saved == 0
    }
}
