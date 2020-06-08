use std::fmt;

// A two-part identifier that has a namespace and name
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct UnlocalizedName {
    pub namespace: String,
    pub name: String
}

impl UnlocalizedName {
    #[inline]
    pub fn minecraft(name: &str) -> UnlocalizedName {
        UnlocalizedName {
            namespace: "minecraft".to_owned(),
            name: name.to_owned()
        }
    }

    pub fn parse(string: &str) -> Result<UnlocalizedName, String> {
        match string.find(':') {
            Some(index) => {
                if index == 0 || index == string.len() - 1 {
                    return Err("Expected two strings separated by a colon.".to_owned());
                } else {
                    Ok(UnlocalizedName {
                        namespace: string[0..index].to_owned(),
                        name: string[index + 1..].to_owned()
                    })
                }
            },
            None => Ok(Self::minecraft(string))
        }
    }
}

impl fmt::Debug for UnlocalizedName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
} 

impl fmt::Display for UnlocalizedName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
}