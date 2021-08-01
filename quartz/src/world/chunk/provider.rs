use crate::world::{
    chunk::{chunk::RawChunk, Chunk},
    location::{Coordinate, CoordinatePair},
};
use byteorder::{BigEndian, ByteOrder};
use flate2::write::{GzDecoder, ZlibDecoder};
use log::{error, warn};
use quartz_nbt::{
    io::NbtIoError,
    serde::deserialize_from_buffer,
};
use quartz_util::hash::NumHasher;
use tokio::{runtime::{Builder, Runtime}, fs::{OpenOptions, File}, io::{AsyncReadExt, AsyncSeekExt, SeekFrom}, sync::{
    RwLock,
    RwLockWriteGuard,
}};
use std::{collections::HashMap, future::Future, io::Write, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicUsize, Ordering}}, io::{self, Error as IoError, ErrorKind}};
use std::collections::hash_map::Entry;

pub struct ChunkProvider {
    pub regions: RegionHandler,
    rt: Runtime
}

impl ChunkProvider {
    /// Creates a chunk provider for the given root directory with the given number of threads.
    pub fn new<P: AsRef<Path>>(
        world_name: String,
        root_directory: P,
    ) -> io::Result<Self> {
        // Ensure the root directory exists
        std::fs::create_dir_all(root_directory.as_ref())?;

        let regions = RegionHandler::new(root_directory.as_ref().to_owned());

        Ok(ChunkProvider {
            regions,
            rt: Builder::new_multi_thread()
                .enable_io()
                .thread_name_fn(move || {
                    static THREAD_ID: AtomicUsize = AtomicUsize::new(0);
                    format!("{}/chunk-provider#{}", world_name, THREAD_ID.fetch_add(1, Ordering::AcqRel))
                })
                .build()?
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

        self.rt
            .spawn(async move {
                let result =
                    Self::handle_request_internal(request.clone(), regions).await;

                if let Err(e) = result {
                    error!("Failed to process request {:?}: {}", request, e);
                }
            });
    }

    async fn handle_request_internal(
        request: ProviderRequest,
        handler: RegionHandler,
    ) -> Result<(), NbtIoError> {
        match request {
            ProviderRequest::LoadFull(coords) => {
                let mut regions = handler.regions_mut().await;
                let region = regions.region_at(coords).await?;

                // The chunk data is still available, so we can just mark it as used and return early
                if region.recover_cached_chunk(coords) {
                    // Drops the guard
                    return Ok(());
                }

                let chunk_nbt = region.load_chunk_nbt(coords).await?;

                drop(region);
                drop(regions);

                match chunk_nbt {
                    Some(chunk_nbt) => {
                        Self::decode_and_cache_chunk(handler, coords, chunk_nbt).await?;
                    }
                    None => {
                        log::warn!("Chunk generation not supported yet.");
                    }
                }

                Ok(())
            }

            ProviderRequest::Unload(coords) => {
                let mut regions = handler.regions_mut().await;

                let region = match regions.loaded_region_at(coords) {
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
                let _region = match regions.remove_region(coords) {
                    Some(region) => region,
                    None => return Ok(()),
                };
                
                drop(regions);

                // TODO: write region to disk
                Ok(())
            }
        }
    }

    async fn decode_and_cache_chunk(handler: RegionHandler, coords: Coordinate, chunk_nbt: Vec<u8>) -> Result<(), NbtIoError> {
        let mut decompressed = Vec::new();

        match chunk_nbt[0] {
            2 => {
                let mut decoder = ZlibDecoder::new(decompressed);
                decoder.write_all(&chunk_nbt[1..])?;
                decompressed = decoder.finish()?;
            },
            // GZip compression (not used in practice)
            1 => {
                let mut decoder = GzDecoder::new(decompressed);
                decoder.write_all(&chunk_nbt[1..])?;
                decompressed = decoder.finish()?;
            },
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

        let mut regions = handler.regions_mut().await;
        match regions.loaded_region_at(chunk.coordinates()) {
            Some(region) => {
                handler.chunks_mut().await.cache_chunk(&mut *region, chunk);
                Ok(())
            },
            None => Err(NbtIoError::Custom(
                String::from("Attempted to cache chunk in an unloaded region")
                    .into_boxed_str(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProviderRequest {
    LoadFull(Coordinate),
    Unload(Coordinate),
}

pub type Map<T> = HashMap<CoordinatePair, T, NumHasher>;

#[derive(Clone)]
pub struct RegionHandler {
    regions: Arc<RwLock<Map<Region>>>,
    chunks: Arc<RwLock<Map<Chunk>>>,
    root_directory: PathBuf,
}

impl RegionHandler {
    fn new(root_directory: PathBuf) -> Self {
        RegionHandler {
            regions: Arc::new(RwLock::new(HashMap::with_hasher(NumHasher))),
            chunks: Arc::new(RwLock::new(HashMap::with_hasher(NumHasher))),
            root_directory,
        }
    }

    pub async fn regions_mut(&self) -> MapGuardMut<'_, Region> {
        MapGuardMut {
            handler: self,
            guard: self.regions.write().await
        }
    }

    pub async fn chunks_mut(&self) -> MapGuardMut<'_, Chunk> {
        MapGuardMut {
            handler: self,
            guard: self.chunks.write().await
        }
    }
}

pub struct MapGuardMut<'a, T> {
    handler: &'a RegionHandler,
    guard: RwLockWriteGuard<'a, Map<T>>
}

impl<'a> MapGuardMut<'a, Region> {
    async fn region_at(&mut self, location: Coordinate) -> io::Result<&mut Region> {
        Ok(self
            .guard
            .entry(location.as_region().into())
            .or_insert(Region::new(&self.handler.root_directory, location).await?))
    }

    #[inline]
    pub fn loaded_region_at(&mut self, location: Coordinate) -> Option<&mut Region> {
        self.guard.get_mut(&location.as_region().into())
    }

    #[inline]
    fn remove_region(&mut self, location: Coordinate) -> Option<Region> {
        self.guard.remove(&location.as_region().into())
    }
}

impl<'a> MapGuardMut<'a, Chunk> {
    pub fn loaded_chunk_at(&mut self, location: Coordinate) -> Option<&mut Chunk> {
        self.guard.get_mut(&location.as_chunk().into())
    }

    fn cache_chunk(&mut self, region: &mut Region, chunk: Chunk) {
        if let Some(chunk_info) = region.mut_chunk_info_at(chunk.coordinates()) {
            chunk_info.cache_inhabited = true;
            chunk_info.cache_active = true;
        }

        let entry = self.guard.entry(chunk.coordinates().as_chunk().into());
        match entry {
            Entry::Occupied(mut cached) => {
                *cached.get_mut() = chunk;
            }
            Entry::Vacant(entry) => {
                entry.insert(chunk);
            },
        }
    }

    pub fn chunk_at(
        &mut self,
        region: &mut Region,
        absolute_position: Coordinate,
    ) -> Option<&mut Chunk> {
        let chunk = self.guard.get_mut(&absolute_position.as_chunk().into());

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
    async fn load_chunk_nbt(
        &mut self,
        absolute_position: Coordinate,
    ) -> Result<Option<Vec<u8>>, NbtIoError> {
        let chunk_info = match self.chunk_info_at(absolute_position) {
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
        let seek_offset = (chunk_info.sector_offset as u64) * 4096;
        self.file
            .seek(SeekFrom::Start(seek_offset))
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
        Ok(Some(buf))
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
