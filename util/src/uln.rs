use serde::{
    de::{self, Visitor},
    Deserialize,
    Serialize,
};
use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    ops::Deref,
    ptr::{self, NonNull},
    slice,
    str,
    str::FromStr,
};

/// This limit ensures that we can store two of this value inside a usize without the resulting
/// usize being greater than `isize::MAX`.
const MAX_ULN_LENGTH: usize = (1 << (usize::BITS / 2 - 1)) - 1;

/// An owned unlocalized name, or a two-part identifier composed of a namespace and identifier
/// separated by a colon.
pub struct UnlocalizedName {
    ptr: NonNull<u8>,
    meta: Meta,
}

// Safety: we are essentially equivalent to Box<[u8]> in representation and ownership semantics, so
// we can safely implement these traits
unsafe impl Send for UnlocalizedName {}
unsafe impl Sync for UnlocalizedName {}

impl UnlocalizedName {
    /// Constructs an unlocalized name from the given raw parts.
    ///
    /// # Safety
    ///
    /// 1. `ptr` must not be aliased or dangling
    /// 2. `ptr` must have been allocated by a `Box`
    /// 3. `ptr` must point to `meta.len()` consecutive bytes of data
    /// 4. The bytes that `ptr` points to must be valid UTF-8
    #[inline]
    const unsafe fn from_raw(ptr: NonNull<u8>, meta: Meta) -> Self {
        UnlocalizedName { ptr, meta }
    }

    /// Returns an owned unlocalized name with namespace "minecraft" and the given identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::uln::UnlocalizedName;
    /// let stone = UnlocalizedName::minecraft("stone");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the length of `ident` cannot fit within half the width of a `usize`. In other
    /// words, `ident.len()` must be less than `1 << (usize::BITS / 2 - 1)`.
    #[inline]
    pub fn minecraft(ident: impl Into<String>) -> Self {
        let repr: String = ident.into();
        let len = match NonZeroUsize::new(repr.len()) {
            Some(len) => {
                if len.get() > MAX_ULN_LENGTH {
                    panic!("{}", ParseUnlocalizedNameError::StringTooLarge(len.get()));
                }

                len
            }
            None => panic!("{}", ParseUnlocalizedNameError::EmptyInput),
        };

        let repr = Box::into_raw(repr.into_boxed_str().into_boxed_bytes()) as *mut u8;

        // Safety: pointer came from a Box and therefore is not null
        let ptr = unsafe { NonNull::new_unchecked(repr) };

        // Safety: `len` is checked above and `colon` is not present
        let meta = unsafe { Meta::new(len, None) };

        // Safety:
        // 1. We obtained `ptr` from an owned value, therefore it is unique and valid
        // 2. `ptr` was obtained via Box::into_raw
        // 3. The length portion of the metadata is equal to the length of the slice from which we
        //    obtained `ptr`
        // 4. The aforementioned slice was obtained from a string, therefore `ptr` points to valid
        //    UTF-8
        unsafe { UnlocalizedName::from_raw(ptr, meta) }
    }

    /// Parses the given string into an unlocalized name and converts it to an owned value. See
    /// `UlnStr::`[`from_str`].
    ///
    /// [`from_str`]: crate::uln::UlnStr::from_str
    pub fn from_str(s: &str) -> Result<Self, ParseUnlocalizedNameError> {
        UlnStr::from_str(s).map(ToOwned::to_owned)
    }

    /// Returns a borrow of this unlocalized name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::uln::UnlocalizedName;
    /// let stone = UnlocalizedName::minecraft("stone");
    /// let borrowed = stone.as_uln_str();
    ///
    /// assert_eq!(stone, borrowed);
    /// ```
    #[inline]
    pub fn as_uln_str(&self) -> &UlnStr {
        &**self
    }
}

impl Serialize for UnlocalizedName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self.meta.colon() {
            None => serializer.serialize_str(&format!("minecraft:{}", self.repr())),
            Some(_) => serializer.serialize_str(self.repr()),
        }
    }
}

impl<'de> Deserialize<'de> for UnlocalizedName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        struct StrVisitor;

        impl<'de> Visitor<'de> for StrVisitor {
            type Value = UnlocalizedName;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "an unlocalized name")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where E: serde::de::Error {
                UnlocalizedName::from_str(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(StrVisitor)
    }
}

impl FromStr for UnlocalizedName {
    type Err = ParseUnlocalizedNameError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str(s)
    }
}

impl From<&UnlocalizedName> for UnlocalizedName {
    #[inline]
    fn from(uln: &UnlocalizedName) -> Self {
        uln.clone()
    }
}

impl From<&UlnStr> for UnlocalizedName {
    #[inline]
    fn from(uln: &UlnStr) -> Self {
        uln.to_owned()
    }
}

impl From<Cow<'_, UlnStr>> for UnlocalizedName {
    #[inline]
    fn from(uln: Cow<'_, UlnStr>) -> Self {
        uln.into_owned()
    }
}

impl<'a> From<&'a UnlocalizedName> for Cow<'a, UlnStr> {
    #[inline]
    fn from(uln: &'a UnlocalizedName) -> Self {
        Cow::Borrowed(&**uln)
    }
}

impl Clone for UnlocalizedName {
    #[inline]
    fn clone(&self) -> Self {
        (**self).to_owned()
    }
}

impl Drop for UnlocalizedName {
    #[inline]
    fn drop(&mut self) {
        let ptr = self.ptr.as_ptr();
        let len = self.meta.len().get();

        // Safety: `ptr` is unique and was allocated by a box by the safety guarantees of the
        // constructors of this type
        let owned_repr = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, len)) };

        drop(owned_repr);
    }
}

impl AsRef<UlnStr> for UnlocalizedName {
    #[inline]
    fn as_ref(&self) -> &UlnStr {
        &**self
    }
}

impl Borrow<UlnStr> for UnlocalizedName {
    #[inline]
    fn borrow(&self) -> &UlnStr {
        &**self
    }
}

impl Display for UnlocalizedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace(), self.identifier())
    }
}

impl Debug for UnlocalizedName {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Deref for UnlocalizedName {
    type Target = UlnStr;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety:
        // 1. Since `self.ptr` is an owning pointer, and since we have a reference to self, we know
        //    that `ptr` won't be dropped until the reference to self goes away.
        // 2. `ptr` is valid for `self.meta.len()` consecutive bytes by the safety guarantees of
        //    the constructors of this type
        // 3. `ptr` points to valid UTF-8 by the safety guarantees of the constructors of this type
        unsafe { UlnStr::new(self.ptr.as_ptr(), self.meta) }
    }
}

impl Hash for UnlocalizedName {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.repr().as_bytes());
    }
}

impl PartialEq for UnlocalizedName {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.meta == other.meta && self.repr() == other.repr()
    }
}

impl Eq for UnlocalizedName {}

impl PartialEq<UlnStr> for UnlocalizedName {
    #[inline]
    fn eq(&self, other: &UlnStr) -> bool {
        self.meta == other.meta() && self.repr() == other.repr()
    }
}

impl PartialEq<&UlnStr> for UnlocalizedName {
    #[inline]
    fn eq(&self, other: &&UlnStr) -> bool {
        self.meta == other.meta() && self.repr() == other.repr()
    }
}

impl PartialEq<Cow<'_, UlnStr>> for UnlocalizedName {
    #[inline]
    fn eq(&self, other: &Cow<'_, UlnStr>) -> bool {
        match other {
            Cow::Borrowed(uln_str) => self == uln_str,
            Cow::Owned(uln) => self == uln,
        }
    }
}

impl PartialOrd for UnlocalizedName {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let namespace_ordering = self.namespace().cmp(other.namespace());

        if namespace_ordering == Ordering::Equal {
            Some(self.identifier().cmp(other.identifier()))
        } else {
            Some(namespace_ordering)
        }
    }
}

impl Ord for UnlocalizedName {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/// A dynamically-sized version of [`UnlocalizedName`]. This type is to `UnlocalizedName` as `str`
/// is to `String`.
///
/// [`UnlocalizedName`]: crate::uln::UnlocalizedName
#[repr(transparent)]
pub struct UlnStr {
    _dst: [()],
}

impl UlnStr {
    /// Constructs a new reference to an unlocalized name from the given pointer and metadata.
    ///
    /// # Safety
    ///
    /// Caller must assert that:
    /// 1. `ptr` is valid for the lifetime `'a`
    /// 2. `ptr` is valid for `meta.len()` consecutive bytes
    /// 3. `ptr` points to valid UTF-8.
    #[inline]
    unsafe fn new<'a>(ptr: *const u8, meta: Meta) -> &'a Self {
        // This is the crux of the hack we use to "construct" an `&UlnStr`. We cast to a slice of
        // the unit type, which is valid for any length, and use the pointer part of the wide
        // pointer as a surrogate for `ptr`, and the length as a surrogate for `meta`.

        // Safety: `ptr` is non-null and not dangling by the contract above. Since the unit type
        // is zero-sized, a slice of it with a valid pointer part is valid for any length. Hence,
        // as far as the computer is concerned, the value of `meta.into()` is just an arbitrary,
        // irrelevant length. Furthermore, the safety guarantees on the constructor for `Meta`
        // ensure that `meta.into() < isize::MAX`.
        let raw = slice::from_raw_parts(ptr as *const (), meta.into());

        // Safety: UlnStr has a single field of type `[()]` and is marked as `repr(transparent)`,
        // hence `&[()]` and `&UlnStr` have the exact same size and alignment.
        &*(raw as *const _ as *const _)
    }

    /// Construct a new reference to an unlocalized name from the given string and colon position.
    /// This method does not perform any checks on `s` or the colon.
    ///
    /// # Safety
    ///
    /// The length of `s` and `colon` must satisfy the safety requirements on `Meta::`[`new`], and
    /// `s` cannot be empty.
    ///
    /// [`new`]: crate::uln::Meta::new
    #[inline]
    unsafe fn from_str_unchecked(s: &str, colon: Option<NonZeroUsize>) -> &Self {
        let ptr = s.as_ptr();
        let meta = Meta::new(NonZeroUsize::new_unchecked(s.len()), colon);

        // Safety: `ptr` is valid because it came from an `&str`. `meta` is valid by the contract
        // above.
        Self::new(ptr, meta)
    }

    /// Attempts to construct a borrow of an unlocalized name in the namespace "minecraft" from the
    /// given string, returning an error if the given string is an invalid unlocalized name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::uln::UlnStr;
    /// let stone = UlnStr::minecraft("stone");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the length of `ident` cannot fit within half the width of a `usize`. In other
    /// words, `ident.len()` must be less than `1 << (usize::BITS / 2 - 1)`.
    #[inline]
    pub fn minecraft(ident: &str) -> &Self {
        if ident.len() > MAX_ULN_LENGTH {
            panic!("{}", ParseUnlocalizedNameError::StringTooLarge(ident.len()));
        }

        if ident.len() == 0 {
            panic!("{}", ParseUnlocalizedNameError::EmptyInput);
        }

        // Safety: the length of `ident` is checked above, and colon is not present
        unsafe { Self::from_str_unchecked(ident, None) }
    }

    /// Parses the given string into a borrowed unlocalized name.
    ///
    /// If the string is not in the form `namespace:identifier` then it is assumed that just an
    /// identifier was provided, and the namespace "minecraft" is used instead. This function will
    /// return an error if the given string has an empty namespace or empty identifier, in other
    /// words the string is in the form `namespace:` or `:identifier`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::uln::UlnStr;
    ///
    /// let stone = UlnStr::from_str("minecraft:stone").unwrap();
    /// assert_eq!(stone.namespace(), "minecraft");
    /// assert_eq!(stone.identifier(), "stone");
    ///
    /// let advancement = UlnStr::from_str("story/mine_diamond").unwrap();
    /// assert_eq!(advancement.namespace(), "minecraft");
    ///
    /// let foobar = UlnStr::from_str("foo:bar").unwrap();
    /// assert_eq!(foobar.namespace(), "foo");
    /// assert_eq!(foobar.identifier(), "bar");
    ///
    /// assert!(UlnStr::from_str(":P").is_err());
    /// ```
    pub fn from_str(s: &str) -> Result<&Self, ParseUnlocalizedNameError> {
        if s.len() > MAX_ULN_LENGTH {
            return Err(ParseUnlocalizedNameError::StringTooLarge(s.len()));
        }

        if s.len() == 0 {
            return Err(ParseUnlocalizedNameError::EmptyInput);
        }

        let index = match s.find(':') {
            Some(index) => index,
            None => return Ok(Self::minecraft(s)),
        };

        if index == s.len() - 1 {
            return Err(ParseUnlocalizedNameError::EmptyIdentifier);
        }

        match NonZeroUsize::new(index) {
            colon @ Some(_) => {
                // Safety: the length of `s` with the namespace included is checked above, and we
                // ensure that `colon < s.len() - 1` above as well.
                if &s[.. index] == "minecraft" {
                    Ok(unsafe { Self::from_str_unchecked(&s[index + 1 ..], None) })
                } else {
                    Ok(unsafe { Self::from_str_unchecked(s, colon) })
                }
            }
            None => Err(ParseUnlocalizedNameError::EmptyNamespace),
        }
    }

    /// Returns the namespace of this unlocalized name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::uln::UlnStr;
    ///
    /// let stone = UlnStr::minecraft("stone");
    /// let custom = UlnStr::from_str("my_namespace:item").unwrap();
    ///
    /// assert_eq!(stone.namespace(), "minecraft");
    /// assert_eq!(custom.namespace(), "my_namespace");
    /// ```
    #[inline]
    pub fn namespace(&self) -> &str {
        let (repr, colon) = self.unpack();
        match colon {
            None => "minecraft",
            // Safety: safe by the safety guarantees of the constructors for this type
            Some(index) => unsafe { repr.get_unchecked(.. index.get()) },
        }
    }

    /// Returns the identifier of this unlocalized name.
    ///
    /// # Examples
    ///
    /// ```
    /// # use quartz_util::uln::UlnStr;
    ///
    /// let stone = UlnStr::minecraft("stone");
    /// let custom = UlnStr::from_str("my_namespace:item").unwrap();
    ///
    /// assert_eq!(stone.identifier(), "stone");
    /// assert_eq!(custom.identifier(), "item");
    /// ```
    #[inline]
    pub fn identifier(&self) -> &str {
        let (repr, colon) = self.unpack();
        match colon {
            None => repr,
            // Safety: safe by the safety guarantees of the constructors for this type
            Some(index) => unsafe { repr.get_unchecked(index.get() + 1 ..) },
        }
    }

    /// Returns the internal string representation of this unlocalized name. The format of this
    /// representation is not guaranteed, but currently it is of the form `namespace:identifier`,
    /// unless `namespace` is `"minecraft"` in which case the representation is simply the
    /// identifier portion of the unlocalized name.
    #[inline]
    pub fn repr(&self) -> &str {
        self.unpack().0
    }

    #[inline]
    fn meta(&self) -> Meta {
        self.unpack_raw().1
    }

    #[inline]
    fn unpack_raw(&self) -> (*const u8, Meta) {
        // Safety: UlnStr has a single field of type `[()]` and is marked as `repr(transparent)`,
        // hence `[()]` and `UlnStr` have the exact same layout.
        let slice = unsafe { &*(self as *const Self as *const [()]) };

        let ptr = slice.as_ptr() as *const u8;

        // Safety: the length of the slice is guaranteed to be a valid Meta by the guarantees of
        // the constructors of this type
        let meta = unsafe { Meta::from_raw(slice.len()) };

        (ptr, meta)
    }

    #[inline]
    fn unpack(&self) -> (&str, Option<NonZeroUsize>) {
        let (ptr, meta) = self.unpack_raw();
        let (len, colon) = meta.unpack();

        // Safety: the safety guarantees of the constructors for this type ensure that `ptr` is
        // valid for `len` consecutive bytes, and points to valid UTF-8.
        let repr = unsafe { str::from_utf8_unchecked(slice::from_raw_parts(ptr, len.get())) };

        (repr, colon)
    }
}

impl Serialize for UlnStr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let (repr, colon) = self.unpack();

        match colon {
            None => serializer.serialize_str(&format!("minecraft:{}", repr)),
            Some(_) => serializer.serialize_str(repr),
        }
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for &'a UlnStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        struct StrVisitor;

        impl<'a> Visitor<'a> for StrVisitor {
            type Value = &'a UlnStr;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a borrowed unlocalized name")
            }

            fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
            where E: serde::de::Error {
                UlnStr::from_str(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(StrVisitor)
    }
}

impl Display for UlnStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let (repr, colon) = self.unpack();
        match colon {
            None => write!(f, "minecraft:{}", repr),
            Some(_) => write!(f, "{}", repr),
        }
    }
}

impl Debug for UlnStr {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl PartialEq for UlnStr {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.meta() == other.meta() && self.repr() == other.repr()
    }
}

impl Eq for UlnStr {}

impl PartialEq<UnlocalizedName> for UlnStr {
    #[inline]
    fn eq(&self, other: &UnlocalizedName) -> bool {
        other == self
    }
}

impl PartialEq<Cow<'_, UlnStr>> for UlnStr {
    #[inline]
    fn eq(&self, other: &Cow<'_, UlnStr>) -> bool {
        match other {
            &Cow::Borrowed(uln_str) => self == uln_str,
            Cow::Owned(uln) => self == uln,
        }
    }
}

impl PartialOrd for UlnStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let namespace_ordering = self.namespace().cmp(other.namespace());

        if namespace_ordering == Ordering::Equal {
            Some(self.identifier().cmp(other.identifier()))
        } else {
            Some(namespace_ordering)
        }
    }
}

impl Ord for UlnStr {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Hash for UlnStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.unpack().0.as_bytes());
    }
}

impl ToOwned for UlnStr {
    type Owned = UnlocalizedName;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        let meta = self.meta();
        let (repr, _) = self.unpack();

        let cloned_repr = Box::into_raw(Box::<[u8]>::from(repr.as_bytes())) as *mut u8;

        // Safety: this is an owning pointer from a box, and therefore cannot be null
        let ptr = unsafe { NonNull::new_unchecked(cloned_repr) };

        // Safety:
        // 1. and 2. `ptr` came from a box and therefore is unique
        // 3. and 4. We obtained the data behind `ptr` and `meta` from a valid unlocalized name
        unsafe { UnlocalizedName::from_raw(ptr, meta) }
    }
}

/// An error when parsing an unlocalized name from a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseUnlocalizedNameError {
    /// Too large of a string was encountered.
    StringTooLarge(usize),
    /// An unlocalized name cannot be empty.
    EmptyInput,
    /// An explicit empty namespace was encountered.
    EmptyNamespace,
    /// An empty identifier was encountered.
    EmptyIdentifier,
}

impl Display for ParseUnlocalizedNameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringTooLarge(len) => write!(
                f,
                "encountered unlocalized name of length {}, the maximum length is {}",
                len, MAX_ULN_LENGTH
            ),
            Self::EmptyInput => write!(f, "unlocalized name cannot be empty"),
            Self::EmptyNamespace => write!(f, "empty namespace in unlocalized name"),
            Self::EmptyIdentifier => write!(f, "empty identifier in unlocalized name"),
        }
    }
}

impl Error for ParseUnlocalizedNameError {}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct Meta(usize);

impl Meta {
    /// Constructs a new metadata object for an unlocalized name from the given length and colon
    /// position.
    ///
    /// # Safety
    ///
    /// Caller must assert that `len` and `colon` (if present) are both less than
    /// `1 << (usize::BITS / 2 - 1)`. If `colon` is not `None`, caller must assert that
    /// `colon < len - 1`.
    #[inline]
    const unsafe fn new(len: NonZeroUsize, colon: Option<NonZeroUsize>) -> Self {
        let colon = match colon {
            None => 0,
            Some(index) => index.get(),
        };

        let raw = (len.get() << (usize::BITS / 2)) | colon;

        // `raw` is guaranteed to be less than `isize::MAX` by the contract with the caller
        Meta::from_raw(raw)
    }

    /// Constructs a new metadata object from a bare usize.
    ///
    /// # Safety
    ///
    /// Caller must assert that `raw < isize::MAX`, that the lower bits of `raw` taken as a
    /// value are less than the upper bits minus one, and at least one of the upper bits is
    /// non-zero.
    ///
    /// `Meta::from_raw(meta.into())` where `meta` is a `Meta` is always safe.
    #[inline]
    const unsafe fn from_raw(raw: usize) -> Self {
        Meta(raw)
    }

    #[inline]
    fn unpack(&self) -> (NonZeroUsize, Option<NonZeroUsize>) {
        (self.len(), self.colon())
    }

    #[inline]
    fn len(&self) -> NonZeroUsize {
        // Safety: guaranteed by the constructors of this type
        unsafe { NonZeroUsize::new_unchecked(self.0.overflowing_shr((usize::BITS / 2) as u32).0) }
    }

    #[inline]
    fn colon(&self) -> Option<NonZeroUsize> {
        NonZeroUsize::new(self.0 & MAX_ULN_LENGTH)
    }
}

impl From<Meta> for usize {
    #[inline]
    fn from(meta: Meta) -> Self {
        meta.0
    }
}

#[test]
fn cloning() {
    let uln = UnlocalizedName::minecraft("dirt");
    let uln2 = uln.clone();

    let uln_str = &*uln;
    let uln3 = uln_str.to_owned();

    assert_eq!(uln, uln2);
    assert_eq!(uln, uln_str);
    assert_eq!(uln, uln3);
}
