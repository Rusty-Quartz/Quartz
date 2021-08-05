use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    ptr,
};

pub const LIGHTING_LENGTH: usize = 2048;
type RawLightBuffer = [u8; LIGHTING_LENGTH];

pub struct Lighting {
    pub block: Option<LightBuffer>,
    pub sky: Option<LightBuffer>,
}

impl Lighting {
    pub const fn new() -> Self {
        Lighting {
            block: None,
            sky: None,
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
    pub data: Box<RawLightBuffer>,
}

impl LightBuffer {
    pub fn new(source: &[u8]) -> Result<Self, LightingInitError> {
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
            data: unsafe { buf.assume_init() },
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LightingInitError {
    InvalidLength(usize),
    AlreadyInitialized,
}

impl Display for LightingInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(len) => write!(
                f,
                "expected light buffer of length {} but found length of {}",
                LIGHTING_LENGTH, len
            ),
            Self::AlreadyInitialized => write!(f, "light buffer already initialized"),
        }
    }
}

impl Error for LightingInitError {}
