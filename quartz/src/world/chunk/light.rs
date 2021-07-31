use std::fmt::{self, Display, Formatter};
use std::ptr;
use std::error::Error;
use crate::network::{ReadFromPacket, WriteToPacket, PacketBuffer, PacketSerdeError};

pub const LIGHTING_LENGTH: usize = 2048;
type RawLightBuffer = [u8; LIGHTING_LENGTH];

pub struct Lighting {
    block: Option<LightBuffer>,
    sky: Option<LightBuffer>
}

impl Lighting {
    pub const fn new() -> Self {
        Lighting {
            block: None,
            sky: None
        }
    }

    #[inline]
    pub fn init_block(&mut self, source: &[u8]) -> Result<(), LightingInitError> {
        Self::init(source, &mut self.block)
    }

    #[inline]
    pub fn init_sky(&mut self, source: &[u8]) -> Result<(), LightingInitError> {
        Self::init(source, &mut self.sky)
    }

    #[inline]
    pub fn has_block_light(&self) -> bool {
        self.block.is_some()
    }

    #[inline]
    pub fn block_light(&self) -> Option<&LightBuffer> {
        self.block.as_ref()
    }

    #[inline]
    pub fn block_light_mut(&mut self) -> Option<&mut LightBuffer> {
        self.block.as_mut()
    }

    #[inline]
    pub fn has_sky_light(&self) -> bool {
        self.sky.is_some()
    }

    #[inline]
    pub fn sky_light(&self) -> Option<&LightBuffer> {
        self.sky.as_ref()
    }

    #[inline]
    pub fn sky_light_mut(&mut self) -> Option<&mut LightBuffer> {
        self.sky.as_mut()
    }

    fn init(source: &[u8], buffer: &mut Option<LightBuffer>) -> Result<(), LightingInitError> {
        if buffer.is_some() {
            return Err(LightingInitError::AlreadyInitialized);
        }

        *buffer = Some(LightBuffer::new(source)?);
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct LightBuffer {
    data: Box<RawLightBuffer>
}

impl LightBuffer {
    fn new(source: &[u8]) -> Result<Self, LightingInitError> {
        if source.len() != LIGHTING_LENGTH {
            return Err(LightingInitError::InvalidLength(source.len()));
        }

        let src = source.as_ptr();
        let mut buf = Box::<RawLightBuffer>::new_uninit();
        let dst = buf.as_mut_ptr() as *mut u8;

        // Safety:
        //  - `src` is valid for `LIGHTING_LENGTH` u8s because of the length check at the beginning
        //    of this function
        //  - `dst` is valid for `LIGHTING_LENGTH` u8s because it points to a `LightBuffer` which
        //    is a [u8; LIGHTING_LENGTH]
        //  - `src` is aligned because it came from a valid slice, and `dst` is aligned because it
        //    came from a Box
        //  - `src` and `dst` do not overlap because `dst` was newly allocated
        unsafe {
            ptr::copy_nonoverlapping(src, dst, LIGHTING_LENGTH);
        }

        Ok(LightBuffer {
            // Safety: we properly initialized the array above
            data: unsafe { buf.assume_init() }
        })
    }
}

impl ReadFromPacket for LightBuffer {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let len: i32 = buffer.read_varying()?;
        if len as usize != LIGHTING_LENGTH {
            return Err(PacketSerdeError::Internal("Found light buffer with length not matching LIGHTING_LENGTH"));
        }

        let remaining = &buffer[buffer.cursor() ..];

        if remaining.len() < LIGHTING_LENGTH {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let light = Self::new(&remaining[.. LIGHTING_LENGTH]).unwrap();
        buffer.set_cursor(buffer.cursor() + LIGHTING_LENGTH);
        Ok(light)
    }
}

impl WriteToPacket for LightBuffer {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write_varying(&(LIGHTING_LENGTH as i32));
        buffer.write_bytes(self.data.as_ref())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LightingInitError {
    InvalidLength(usize),
    AlreadyInitialized
}

impl Display for LightingInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(len) => write!(f, "expected light buffer of length {} but found length of {}", LIGHTING_LENGTH, len),
            Self::AlreadyInitialized => write!(f, "light buffer already initialized")
        }
    }
}

impl Error for LightingInitError {}