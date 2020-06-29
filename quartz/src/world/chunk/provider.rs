use std::collections::HashMap;
use std::io::{self, Read};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use crate::world::{Chunk, CoordinatePair};

pub struct ChunkProvider {
    root_directory: PathBuf,
    regions: HashMap<CoordinatePair, Region>
}

impl ChunkProvider {
    pub fn new(root_directory: &Path) -> io::Result<Self> {
        // Ensure the root directory exists
        fs::create_dir_all(root_directory)?;

        Ok(ChunkProvider {
            root_directory: root_directory.to_owned(),
            regions: HashMap::new()
        })
    }
}

struct Region {
    file: File,
    offset: CoordinatePair,
    chunk_info: Box<[ChunkDataInfo]>
}

impl Region {
    pub fn new(root_directory: &Path, location: CoordinatePair) -> io::Result<Self> {
        let file_path = root_directory.join(format!("r.{}.{}.mca", location.x, location.z));

        let mut chunk_info: Vec<ChunkDataInfo> = Vec::with_capacity(1024);
        for _ in 0..1024 {
            chunk_info.push(ChunkDataInfo::uninitialized());
        }

        if file_path.exists() {
            let mut region = Region {
                file: OpenOptions::new().read(true).write(true).open(file_path)?,
                offset: location.scaled_up(32),
                chunk_info: chunk_info.into_boxed_slice()
            };
            region.read_file_header()?;

            Ok(region)
        } else {
            Ok(Region {
                file: File::create(file_path)?,
                offset: location.scaled_up(32),
                chunk_info: chunk_info.into_boxed_slice()
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
        let mut chunk_info: &mut ChunkDataInfo;
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
            chunk_info.last_saved = (buffer[j] as u32) << 24 |(buffer[j + 1] as u32) << 16 | (buffer[j + 2] as u32) << 8 | (buffer[j + 3] as u32);
        }

        Ok(())
    }
}

struct ChunkDataInfo {
    pub sector_offset: u32,
    pub sector_count: u8,
    pub last_saved: u32,
    pub cached_chunk: Option<Box<Chunk>>
}

impl ChunkDataInfo {
    pub const fn uninitialized() -> Self {
        ChunkDataInfo {
            sector_offset: 0,
            sector_count: 0,
            last_saved: 0,
            cached_chunk: None
        }
    }
}