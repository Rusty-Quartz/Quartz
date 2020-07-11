use std::fmt::{self, Debug, Display, Formatter};

/// An unlocalized name is a two-part identifier composed of a namespace and identifier separated
/// by a colon.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct UnlocalizedName {
    /// The namespace of this unlocalized name.
    pub namespace: String,
    /// The identifier portion of this unlocalized name.
    pub identifier: String
}

impl UnlocalizedName {
    /// Returns an unlocalized name with namespace "minecraft" and the given identifier.
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use util::UnlocalizedName;
    /// let stone = UnlocalizedName::minecraft("stone");
    /// 
    /// assert_eq!(stone.namespace, "minecraft");
    /// assert_eq!(stone.identifier, "stone");
    /// ```
    #[inline]
    pub fn minecraft(identifier: &str) -> UnlocalizedName {
        UnlocalizedName {
            namespace: "minecraft".to_owned(),
            identifier: identifier.to_owned()
        }
    }

    /// Parses the given string into an unlocalized name.
    /// 
    /// If the string is not in the form `namespace:identifier` then it is assumed that just an
    /// identifier was provided, and the namespace "minecraft" is used instead. This function will
    /// return an error if the given string has an empty namespace or empty identifier, in other
    /// words the string is in the form `namespace:` or `:identifier`.
    /// 
    /// # Examples
    /// 
    /// ```
    /// # use util::UnlocalizedName;
    /// let stone = UnlocalizedName::parse("minecraft:stone").unwrap();
    /// assert_eq!(stone.namespace, "minecraft");
    /// assert_eq!(stone.identifier, "stone");
    /// 
    /// let advancement = UnlocalizedName::parse("story/mine_diamond").unwrap();
    /// assert_eq!(advancement.namespace, "minecraft");
    /// 
    /// let foobar = UnlocalizedName::parse("foo:bar").unwrap();
    /// assert_eq!(foobar.namespace, "foo");
    /// assert_eq!(foobar.identifier, "bar");
    /// 
    /// assert!(UnlocalizedName::parse(":P").is_err());
    /// ```
    pub fn parse(string: &str) -> Result<UnlocalizedName, &'static str> {
        let index = match string.find(':') {
            Some(index) => index,
            None => return Ok(Self::minecraft(string))
        };

        if index == 0 || index == string.len() - 1 {
            Err("Expected two strings separated by a colon.")
        } else {
            Ok(UnlocalizedName {
                namespace: string[0..index].to_owned(),
                identifier: string[index + 1..].to_owned()
            })
        }
    }
}

impl Display for UnlocalizedName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.identifier)
    }
}

impl Debug for UnlocalizedName {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}