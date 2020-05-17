use std::fmt;

// A two-part identifier that has a namespace and name
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct UnlocalizedName<'a> {
    namespace: &'a str,
    name: &'a str
}

impl<'a> UnlocalizedName<'a> {
    #[inline]
    pub const fn minecraft(name: &'static str) -> UnlocalizedName<'static> {
        UnlocalizedName {
            namespace: "minecraft",
            name
        }
    }

    pub fn parse(string: &'a str) -> Result<UnlocalizedName<'a>, String> {
        match string.find(':') {
            Some(index) => {
                if index == 0 || index == string.len() - 1 {
                    return Err("Expected two strings separated by a colon.".to_owned());
                } else {
                    Ok(UnlocalizedName {
                        namespace: &string[0..index],
                        name: &string[index + 1..]
                    })
                }
            },
            None => Err("Expected ':' in unlocalized name.".to_owned())
        }
    }
}


impl<'a> fmt::Display for UnlocalizedName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
}