use crate::world::{
    chunk::{chunk::RawChunk, Chunk},
    location::{Coordinate, CoordinatePair},
};
use byteorder::{BigEndian, ByteOrder};
use dashmap::{
    mapref::{
        entry::Entry,
        one::{Ref, RefMut},
    },
    DashMap,
};
use futures_lite::{
    future,
    io::{AsyncReadExt, AsyncSeekExt},
    FutureExt,
};
use log::{error, warn};
use quartz_nbt::{
    io::{read_nbt, Flavor, NbtIoError},
    serde::deserialize,
};
use quartz_util::hash::NumHasher;
use smol::{
    channel::{self, Receiver, Recv, Sender},
    fs::{File, OpenOptions},
    io::{self, Error as IoError, ErrorKind, SeekFrom},
    lock::{Mutex, MutexGuard},
    Executor,
};
use std::{
    collections::HashMap,
    future::Future,
    marker::PhantomData,
    path::{Path, PathBuf},
    pin::Pin,
    ptr::NonNull,
    sync::Arc,
    task::{Context, Poll},
    thread,
};

pub struct ChunkProvider {
    pub regions: RegionHandler,
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
    ) -> io::Result<Self> {
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

    pub fn load_full(
        &self,
        coordinates: Coordinate,
    ) -> impl Future<Output = Result<MapRefMut<'_, Chunk>, NbtIoError>> {
        async move {
            let regions = self.regions.clone();

            let result = self
                .executor
                .spawn(async move {
                    let request = ProviderRequest::LoadFull(coordinates);
                    let (chunk_tx, chunk_rx) = channel::bounded::<Chunk>(1);

                    let result =
                        Self::handle_request_internal(request.clone(), regions, chunk_tx.clone())
                            .await;

                    let result = match result {
                        Ok(_) => Ok(chunk_rx.recv().await.unwrap()),
                        Err(e) => Err(e),
                    };

                    drop(chunk_tx);
                    result
                })
                .await;

            match result {
                Ok(chunk) => match self.regions.loaded_region_at(chunk.coordinates()) {
                    Some(mut region) => Ok(self.regions.cache_chunk(&mut *region, chunk)),
                    None => Err(NbtIoError::Custom(
                        String::from("Attempted to cache chunk in an unloaded region")
                            .into_boxed_str(),
                    )),
                },
                Err(e) => Err(e),
            }
        }
    }

    pub fn request_load_full(&self, coordinates: Coordinate) {
        self.handle_request(ProviderRequest::LoadFull(coordinates));
    }

    pub fn flush_queue(&self) {
        let mut rec_chunks = 0;

        while let Ok(chunk) = self.chunk_receiver.try_recv() {
            match self.regions.loaded_region_at(chunk.coordinates()) {
                Some(mut region) => {
                    rec_chunks += 1;
                    self.regions.cache_chunk(&mut *region, chunk);
                }
                // This should never happen
                None => drop(chunk),
            }
        }
        if rec_chunks != 0 {
            log::debug!("recieved {} chunks", rec_chunks);
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
        handler: RegionHandler,
        chunk_sender: Sender<Chunk>,
    ) -> Result<(), NbtIoError> {
        match request {
            ProviderRequest::LoadFull(coords) => {
                let mut region = handler.region_at(coords).await?;

                // The chunk data is still available, so we can just mark it as used and return early
                if region.recover_cached_chunk(coords) {
                    // Drops the guard
                    return Ok(());
                }

                let chunk = region.load_chunk_nbt(coords).await?;

                drop(region);

                match chunk {
                    Some(chunk) => {
                        chunk_sender.send(chunk).await.map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "Failed to send chunk between threads",
                            )
                        })?;
                    }
                    None => {
                        log::warn!("Chunk generation not supported yet.");
                    }
                }
            }

            ProviderRequest::Unload(coords) => {
                let mut region = match handler.loaded_region_at(coords) {
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
                let region = match handler.remove_region(coords) {
                    Some(region) => region,
                    None => return Ok(()),
                };

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

type MapKey = CoordinatePair;
type Map<T> = DashMap<MapKey, T, NumHasher>;
type MapRef<'a, T> = Ref<'a, MapKey, T, NumHasher>;
type MapRefMut<'a, T> = RefMut<'a, MapKey, T, NumHasher>;

#[derive(Clone)]
pub struct RegionHandler {
    regions: Arc<Map<Region>>,
    chunks: Arc<Map<Chunk>>,
    root_directory: PathBuf,
}

impl RegionHandler {
    fn new(root_directory: PathBuf) -> Self {
        RegionHandler {
            regions: Arc::new(DashMap::with_hasher(NumHasher)),
            chunks: Arc::new(DashMap::with_hasher(NumHasher)),
            root_directory,
        }
    }

    async fn region_at(&self, location: Coordinate) -> io::Result<MapRefMut<'_, Region>> {
        Ok(self
            .regions
            .entry(location.as_region().into())
            .or_insert(Region::new(&self.root_directory, location).await?))
    }

    #[inline]
    pub fn loaded_region_at(&self, location: Coordinate) -> Option<MapRefMut<'_, Region>> {
        self.regions.get_mut(&location.as_region().into())
    }

    #[inline]
    fn remove_region(&self, location: Coordinate) -> Option<Region> {
        self.regions
            .remove(&location.as_region().into())
            .map(|(_, region)| region)
    }

    pub fn loaded_chunk_at(&self, location: Coordinate) -> Option<MapRefMut<'_, Chunk>> {
        self.chunks.get_mut(&location.as_chunk().into())
    }

    fn cache_chunk(&self, region: &mut Region, chunk: Chunk) -> MapRefMut<'_, Chunk> {
        if let Some(chunk_info) = region.mut_chunk_info_at(chunk.coordinates()) {
            chunk_info.cache_inhabited = true;
            chunk_info.cache_active = true;
        }

        let mut entry = self.chunks.entry(chunk.coordinates().as_chunk().into());
        match entry {
            Entry::Occupied(mut cached) => {
                *cached.get_mut() = chunk;
                cached.into_ref()
            }
            Entry::Vacant(entry) => entry.insert(chunk),
        }
    }

    pub fn chunk_at(
        &self,
        region: &mut Region,
        absolute_position: Coordinate,
    ) -> Option<MapRefMut<'_, Chunk>> {
        let chunk = self.chunks.get_mut(&absolute_position.as_chunk().into());

        match chunk {
            Some(chunk) => {
                let chunk_info = match region.mut_chunk_info_at(absolute_position) {
                    Some(info) => info,
                    None => {
                        warn!(
                            "Failed to extract chunk info from region at {:?}",
                            absolute_position
                        );
                        return None;
                    }
                };

                if !chunk_info.cache_active {
                    chunk_info.cache_active = true;
                    region.loaded_count += 1;
                }

                Some(chunk)
            }

            None => None,
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
        let chunk_offset: CoordinatePair = location.as_region().as_chunk().into();
        let region_offset: CoordinatePair = location.as_region().into();

        let file_path =
            root_directory.join(format!("r.{}.{}.mca", region_offset.x, region_offset.z));

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
            + (absolute_position.z - self.chunk_offset.z) * 32) as usize
    }

    fn chunk_info_at(&self, absolute_position: Coordinate) -> Option<&ChunkDataInfo> {
        self.chunk_info
            .get(self.index_absolute(absolute_position.as_chunk().into()))
    }

    fn mut_chunk_info_at(&mut self, absolute_position: Coordinate) -> Option<&mut ChunkDataInfo> {
        self.chunk_info
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
    ) -> Result<Option<Chunk>, NbtIoError> {
        let chunk_info = match self
            .chunk_info
            .get_mut(self.index_absolute(absolute_position.as_chunk().into()))
        {
            Some(chunk_info) => chunk_info,
            None =>
                return Err(IoError::new(
                    ErrorKind::InvalidInput,
                    "Attempted to load chunk outside of region",
                )
                .into()),
        };

        if chunk_info.is_uninitialized() {
            return Ok(None);
        }

        // The sector offset accounts for the tables at the beginning
        self.file
            .seek(SeekFrom::Start((chunk_info.sector_offset as u64) * 4096))
            .await?;

        // Read the length
        let mut buf = [0u8; 4];
        self.file.read_exact(&mut buf).await?;
        let length = BigEndian::read_u32(&buf) as usize;

        let mut buf: Vec<u8> = Vec::with_capacity(length);
        unsafe {
            buf.set_len(length);
        }

        self.file.read_exact(&mut buf).await?;

        let nbt = read_nbt(
            &mut std::io::Cursor::new(&buf[1 ..]),
            Flavor::ZlibCompressed,
        )?
        .0;
        std::fs::write("./nbt_data", nbt.to_snbt()).unwrap();

        let (chunk, _) = match buf[0] {
            // GZip compression (not used in practice)
            1 => deserialize::<RawChunk>(&buf[1 ..], Flavor::GzCompressed)?,
            2 => deserialize::<RawChunk>(&buf[1 ..], Flavor::ZlibCompressed)?,
            _ =>
                return Err(IoError::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Encountered invalid compression scheme ({}) for chunk at {}",
                        buf[0],
                        absolute_position.as_chunk()
                    ),
                )
                .into()),
        };

        Ok(Some(chunk.into()))
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
