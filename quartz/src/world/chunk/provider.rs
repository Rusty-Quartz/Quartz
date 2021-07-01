use crate::{
    world::{
        chunk::Chunk,
        location::{Coordinate, CoordinatePair},
    },
};
use futures_lite::{
    future,
    io::{AsyncReadExt, AsyncSeekExt},
};
use log::{error, warn};
use quartz_nbt::read::{read_nbt_gz_compressed, read_nbt_zlib_compressed};
use smol::{
    channel::{self, Receiver, Sender},
    fs::{File, OpenOptions},
    io::{self, Error as IoError, ErrorKind, SeekFrom},
    lock::{Mutex, MutexGuard},
    Executor,
};
use quartz_util::hash::NumHasher;
use std::{collections::HashMap, io::Cursor as StdCursor, path::{Path, PathBuf}, sync::Arc, thread};

pub struct ChunkProvider {
    regions: RegionHandler,
    chunk_sender: Sender<Chunk>,
    chunk_receiver: Receiver<Chunk>,
    executor: Arc<Executor<'static>>,
    _shutdown_signal: Sender<()>,
}

impl ChunkProvider {
    /// Creates a chunk provider for the given root directory with the given number of threads.
    pub fn new<P: AsRef<Path>>(
        world_name: &str,
        root_directory: P,
        thread_count: usize,
    ) -> io::Result<Self>
    {
        // Ensure the root directory exists
        std::fs::create_dir_all(root_directory.as_ref())?;

        let regions = RegionHandler::new(root_directory.as_ref().to_owned());
        let (chunk_sender, chunk_receiver) = channel::unbounded::<Chunk>();
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

    pub fn request_load_full(&self, coordinates: Coordinate) {
        self.handle_request(ProviderRequest::LoadFull(coordinates));
    }

    pub async fn flush_queue(&mut self) {
        let mut region_guard = self.regions.lock_regions().await;
        let mut chunk_guard = self.regions.lock_chunks().await;

        while let Ok(chunk) = self.chunk_receiver.try_recv() {
            match region_guard.loaded_region_at(chunk.coordinates()) {
                Some(region) => chunk_guard.cache_chunk(region, chunk),
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
        regions: RegionHandler,
        chunk_sender: Sender<Chunk>,
    ) -> io::Result<()>
    {
        match request {
            ProviderRequest::LoadFull(coords) => {
                let mut region_guard = regions.lock_regions().await;
                let region = region_guard.region_at(coords).await?;

                // The chunk data is still available, so we can just mark it as used and return early
                if region.recover_cached_chunk(coords) {
                    // Drops the guard
                    return Ok(());
                }

                let mut buffer: Vec<u8> = Vec::new();
                let saved_on_disk = region.load_chunk_nbt(coords, &mut buffer).await?;

                drop(region);
                drop(region_guard);

                if saved_on_disk {
                    let (nbt, _) = match buffer[0] {
                        // GZip compression (not used in practice)
                        1 => read_nbt_gz_compressed(&mut StdCursor::new(&buffer[1 ..]))?,
                        2 => read_nbt_zlib_compressed(&mut StdCursor::new(&buffer[1 ..]))?,
                        _ =>
                            return Err(IoError::new(
                                ErrorKind::InvalidData,
                                format!(
                                    "Encountered invalid compression scheme ({}) for chunk at {}",
                                    buffer[0], coords
                                ),
                            )),
                    };

                    let chunk = Chunk::from_nbt(&nbt);
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
                                format!("Encountered invalid NBT for chunk at {}: {}", coords, e),
                            )
                            .into()),
                    }
                } else {
                    log::warn!("Chunk generation not supported yet.");
                }
            }

            ProviderRequest::Unload(coords) => {
                let mut region_guard = regions.lock_regions().await;
                let region = match region_guard.loaded_region_at(coords) {
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
                let region = match region_guard.remove_region(coords) {
                    Some(region) => region,
                    None => return Ok(()),
                };
                drop(region_guard);

                let mut chunk_guard = regions.lock_chunks().await;
                // TODO: write region to disk
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum ProviderRequest {
    LoadFull(Coordinate),
    Unload(Coordinate),
}

// TODO: Consider changing to RwLock in the future
type Map<T> = HashMap<CoordinatePair, T, NumHasher>;

#[derive(Clone)]
pub struct RegionHandler {
    regions: Arc<Mutex<Map<Region>>>,
    chunks: Arc<Mutex<Map<Chunk>>>,
    root_directory: PathBuf,
}

impl RegionHandler {
    fn new(root_directory: PathBuf) -> Self {
        RegionHandler {
            regions: Arc::new(Mutex::new(HashMap::with_hasher(NumHasher))),
            chunks: Arc::new(Mutex::new(HashMap::with_hasher(NumHasher))),
            root_directory,
        }
    }

    async fn lock_regions(&self) -> RegionHandlerGuard<'_, Region> {
        RegionHandlerGuard {
            guard: self.regions.lock().await,
            map: self,
        }
    }

    async fn lock_chunks(&self) -> RegionHandlerGuard<'_, Chunk> {
        RegionHandlerGuard {
            guard: self.chunks.lock().await,
            map: self
        }
    }
}

pub struct RegionHandlerGuard<'a, T> {
    guard: MutexGuard<'a, Map<T>>,
    map: &'a RegionHandler,
}

impl<'a> RegionHandlerGuard<'a, Region> {
    async fn region_at(&mut self, location: Coordinate) -> io::Result<&mut Region> {
        Ok(self
            .guard
            .entry(location.as_region().into())
            .or_insert(Region::new(&self.map.root_directory, location).await?))
    }

    pub fn loaded_region_at(&mut self, location: Coordinate) -> Option<&mut Region> {
        self.guard.get_mut(&location.as_region().into())
    }

    fn remove_region(&mut self, location: Coordinate) -> Option<Region> {
        self.guard.remove(&location.as_region().into())
    }
}

impl<'a> RegionHandlerGuard<'a, Chunk> {
    pub fn loaded_chunk_at(&mut self, location: Coordinate) -> Option<&mut Chunk> {
        self.guard.get_mut(&location.as_chunk().into())
    }

    fn cache_chunk(&mut self, region: &mut Region, chunk: Chunk) {
        if let Some(chunk_info) = region.mut_chunk_info_at(chunk.coordinates()) {
            chunk_info.cache_inhabited = true;
            chunk_info.cache_active = true;
        }

        self.guard.insert(chunk.coordinates().as_chunk().into(), chunk);
    }

    pub fn chunk_at(&mut self, region: &mut Region, absolute_position: Coordinate) -> Option<&mut Chunk> {
        let chunk = self.guard.get_mut(&absolute_position.as_chunk().into());

        match chunk {
            Some(chunk) => {
                let chunk_info = match region.mut_chunk_info_at(absolute_position) {
                    Some(info) => info,
                    None => {
                        warn!("Failed to extract chunk info from region at {:?}", absolute_position);
                        return None;
                    }
                };
                
                if !chunk_info.cache_active {
                    chunk_info.cache_active = true;
                    region.loaded_count += 1;
                }

                Some(chunk)
            },

            None => None
        }
    }
}

pub struct Region {
    file: File,
    chunk_offset: CoordinatePair,
    loaded_count: usize,
    chunk_info: Box<[ChunkDataInfo]>,
}

impl Region {
    async fn new(root_directory: &Path, location: Coordinate) -> io::Result<Self> {
        let chunk_offset: CoordinatePair = location.as_chunk().into();
        let file_path = root_directory.join(format!("r.{}.{}.mca", chunk_offset.x, chunk_offset.z));

        let mut chunk_info: Vec<ChunkDataInfo> = Vec::with_capacity(1024);
        chunk_info.resize_with(1024, ChunkDataInfo::uninitialized);

        if file_path.exists() {
            let mut region = Region {
                file: OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(file_path)
                    .await?,
                chunk_offset,
                chunk_info: chunk_info.into_boxed_slice(),
                loaded_count: 0,
            };
            region.read_file_header().await?;

            Ok(region)
        } else {
            Ok(Region {
                file: File::create(file_path).await?,
                chunk_offset,
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

        let mut buffer = vec![0; 8192].into_boxed_slice();
        self.file.read_exact(&mut *buffer).await?;

        let mut j: usize;
        let mut chunk_info: &mut ChunkDataInfo;
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
            + (absolute_position.z - self.chunk_offset.z) * 16) as usize
    }

    fn chunk_info_at(&self, absolute_position: Coordinate) -> Option<&ChunkDataInfo> {
        self
            .chunk_info
            .get(self.index_absolute(absolute_position.as_chunk().into()))
    }

    fn mut_chunk_info_at(&mut self, absolute_position: Coordinate) -> Option<&mut ChunkDataInfo> {
        self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
    }

    fn recover_cached_chunk(&mut self, absolute_position: Coordinate) -> bool {
        let chunk = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
        {
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
    async fn load_chunk_nbt(
        &mut self,
        absolute_position: Coordinate,
        buffer: &mut Vec<u8>,
    ) -> io::Result<bool>
    {
        let chunk_info = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
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

    fn mark_chunk_inactive(&mut self, absolute_position: Coordinate) {
        let chunk = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
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
struct ChunkDataInfo {
    sector_offset: u32,
    last_saved: u32,
    sector_count: u8,
    /// Whether or not the corresponding chunk is cached.
    cache_inhabited: bool,
    /// Whether or not the cached chunk value is actually being accessed. If this is set to false, then the region
    /// that contains this chunk data may be unloaded at any time if no other chunks are loaded.
    cache_active: bool,
}

impl ChunkDataInfo {
    pub fn uninitialized() -> Self {
        ChunkDataInfo {
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
