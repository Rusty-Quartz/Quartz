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
    #[inline]
    pub fn minecraft(identifier: &str) -> UnlocalizedName {
        UnlocalizedName {
            namespace: "minecraft".to_owned(),
            identifier: identifier.to_owned()
        }
    }

    /// Parses the given string into an unlocalized name. If the string is not in the form
    /// `namespace:identifier` then it is assumed that just an identifier was provided, and
    /// the namespace "minecraft" is used instead. This function will return an error if the
    /// given string has an empty namespace or empty identifier, in other words the string is
    /// in the form `namespace:` or `:identifier`.
    pub fn parse(string: &str) -> Result<UnlocalizedName, String> {
        match string.find(':') {
            Some(index) => {
                if index == 0 || index == string.len() - 1 {
                    return Err("Expected two strings separated by a colon.".to_owned());
                } else {
                    Ok(UnlocalizedName {
                        namespace: string[0..index].to_owned(),
                        identifier: string[index + 1..].to_owned()
                    })
                }
            },
            None => Ok(Self::minecraft(string))
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