use crate::NbtCompound;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::hash::Hash;
use std::option::NoneError;
use std::str::FromStr;

/// Defines a type which has a full representation as a [`NbtCompound`].
///
/// Full representation meaning that the type can be constructed from a [`NbtCompound`], and fully serialized
/// as one as well.
///
/// [`NbtCompound`]: crate::tag::NbtCompound
pub trait NbtRepr: Sized {
    /// The error type returned if the [`from_nbt`] function fails.
    ///
    /// It is recommended (but not required) that this type implement [`From`]`<`[`NoneError`]`>`
    /// so that other functions in this library can seemlessly interact with this trait.
    ///
    /// [`From`]: std::convert::From
    /// [`NoneError`]: std::option::NoneError
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
