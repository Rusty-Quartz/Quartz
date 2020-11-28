use crate::NbtCompound;
use std::{
    collections::{BTreeMap, HashMap},
    convert::TryInto,
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    hash::Hash,
    option::NoneError,
    str::FromStr,
};

/// An error associated with the structure of an NBT tag tree. This error represents a conflict
/// between the expected and actual structure of an NBT tag tree.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NbtStructureError {
    /// The expected type of a tag was not the type encountered.
    TypeMismatch,
    /// An index was out of bounds.
    InvalidIndex,
    /// A tag in a [`NbtCompound`](crate::tag::NbtCompound) was absent.
    MissingTag,
}

impl Display for NbtStructureError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Error for NbtStructureError {}

impl From<NbtReprError<NbtStructureError>> for NbtStructureError {
    fn from(x: NbtReprError<NbtStructureError>) -> Self {
        match x {
            NbtReprError::Structure(e) => e,
            NbtReprError::Conversion(e) => e,
        }
    }
}

/// An error assocaited with the translation of a NBT representation to a concrete type. This
/// can either be a structre error or a custom error.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NbtReprError<E> {
    /// And error associated with the NBT tree itself. See [`NbtStructureError`](crate::repr::NbtStructureError).
    Structure(NbtStructureError),
    /// A custom error defining an issue during the conversion process.
    Conversion(E),
}

impl<E> NbtReprError<E> {
    /// Creates a [`Conversion`](crate::repr::NbtReprError::Conversion) variant of this error with
    /// the given error.
    pub fn conversion(x: E) -> Self {
        NbtReprError::Conversion(x)
    }
}

impl<E: Debug> Display for NbtReprError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl<E: Error + 'static> Error for NbtReprError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NbtReprError::Structure(source) => Some(source),
            NbtReprError::Conversion(source) => Some(source),
        }
    }
}

impl<E> From<NbtStructureError> for NbtReprError<E> {
    fn from(x: NbtStructureError) -> Self {
        NbtReprError::Structure(x)
    }
}

/// Defines a type which has a full representation as a [`NbtCompound`].
///
/// Full representation meaning that the type can be constructed from a [`NbtCompound`], and fully serialized
/// as one as well.
///
/// [`NbtCompound`]: crate::tag::NbtCompound
pub trait NbtRepr: Sized {
    /// The error type returned if the [`from_nbt`] function fails.
    ///
    /// [`from_nbt`]: crate::repr::NbtRepr::from_nbt
    type Error;

    /// Creates an instance of this type from the given compound.
    ///
    /// The intention is that data is copied, not moved, from the compound to construct this type. If for
    /// whatever reason this type cannot be properly constructed from the given compound, `None` should
    /// be returned.
    fn from_nbt(nbt: &NbtCompound) -> Result<Self, Self::Error>;

    /// Writes all necessary data to the given compound to serialize this type.
    ///
    /// Although not enforced, the data written should allow for the type to be reconstructed via the
    /// [`from_nbt`] function.
    ///
    /// [`from_nbt`]: crate::repr::NbtRepr::from_nbt
    fn write_nbt(&self, nbt: &mut NbtCompound);

    /// Converts this type into an owned [`NbtCompound`].
    ///
    /// Currently this is just a wrapper around creating an empty compound, proceeding to call [`write_nbt`] on
    /// a mutable reference to that compound, then returning the compound.
    ///
    /// [`NbtCompound`]: crate::tag::NbtCompound
    /// [`write_nbt`]: crate::repr::NbtRepr::write_nbt
    #[inline]
    fn to_nbt(&self) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        self.write_nbt(&mut nbt);
        nbt
    }
}

impl<K, V> NbtRepr for BTreeMap<K, V>
where
    K: ToString + FromStr + Ord,
    V: NbtRepr,
{
    type Error = NoneError;

    fn from_nbt(nbt: &NbtCompound) -> Result<Self, Self::Error> {
        let mut map: BTreeMap<K, V> = BTreeMap::new();
        for (key, tag) in nbt.as_ref().iter() {
            map.insert(
                K::from_str(key).ok()?,
                V::from_nbt(tag.try_into().ok()?).map_err(|_| NoneError)?,
            );
        }
        Ok(map)
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        for (key, value) in self.iter() {
            nbt.set(key.to_string(), value.to_nbt());
        }
    }

    #[inline]
    fn to_nbt(&self) -> NbtCompound {
        let mut nbt = NbtCompound::with_capacity(self.len());
        self.write_nbt(&mut nbt);
        nbt
    }
}

impl<K, V> NbtRepr for HashMap<K, V>
where
    K: ToString + FromStr + Eq + Hash,
    V: NbtRepr,
{
    type Error = NoneError;

    fn from_nbt(nbt: &NbtCompound) -> Result<Self, Self::Error> {
        let mut map: HashMap<K, V> = HashMap::new();
        for (key, tag) in nbt.as_ref().iter() {
            map.insert(
                K::from_str(key).ok()?,
                V::from_nbt(tag.try_into().ok()?).map_err(|_| NoneError)?,
            );
        }
        Ok(map)
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        for (key, value) in self.iter() {
            nbt.set(key.to_string(), value.to_nbt());
        }
    }

    #[inline]
    fn to_nbt(&self) -> NbtCompound {
        let mut nbt = NbtCompound::with_capacity(self.len());
        self.write_nbt(&mut nbt);
        nbt
    }
}
