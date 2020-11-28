use crate::*;
use std::{iter::Peekable, str::Chars};

macro_rules! parse_nbt_array {
    ($enum_name: ident, $vec_type: ty, $output_type: ident, $self_val: ident) => {
        match $self_val.data.next() {
            Some(';') => {
                $self_val.handle_whitespace();
                let mut output: Vec<$vec_type> = Vec::new();
                loop {
                    $self_val.handle_whitespace();
                    match $self_val.parse_value() {
                        Ok(NbtTag::$enum_name(v)) => {
                            output.push(v);
                        }
                        Err(e) => return Err(e),
                        Ok(v) =>
                            return Err(format!(
                                "Cannot insert {} into array of type {}",
                                v.type_string(),
                                stringify!($enum_name)
                            )),
                    }

                    $self_val.handle_whitespace();

                    match $self_val.data.next() {
                        Some(']') => return Ok(NbtTag::$output_type(output)),
                        Some(',') => {}
                        Some(_) => return Err("Expected ,".to_owned()),
                        None => return Err("Unclosed array".to_owned()),
                    }
                }
            }
            _ => return Err("stuff".to_owned()),
        }
    };
}

/// Parses SNBT into NBT tags
///
/// # Example
/// ```
/// # use nbt::snbt::*;
/// let mut parser = SnbtParser::new(r#"{string:Stuff, list:[I;1,2,3,4,5]}"#, 0);
/// let tag = parser.parse().unwrap();
///
/// assert_eq!(tag.get::<&str>("string"), Ok("Stuff"));
/// assert_eq!(tag.get::<&[i32]>("list"), Ok(vec![1,2,3,4,5].as_slice()));
/// ```
pub struct SnbtParser<'sp> {
    data: Peekable<Chars<'sp>>,
    /// The current position the parser is at in the data.
    pub cursor: usize,
}

impl<'sp> SnbtParser<'sp> {
    /// Creates a new SNBT parser over the given string.
    pub fn new(data: &'sp str, offset: usize) -> Self {
        SnbtParser {
            data: data.chars().peekable(),
            cursor: offset,
        }
    }

    /// Parses the SNBT data into a NBT compound.
    pub fn parse(&mut self) -> Result<NbtCompound, String> {
        let root_tag = NbtCompound::new();

        self.parse_compound_tag(root_tag)
    }

    fn parse_compound_tag(&mut self, mut tag: NbtCompound) -> Result<NbtCompound, String> {
        match self.data.next() {
            Some('{') => {
                match self.data.peek() {
                    Some('}') => return Ok(tag),
                    None => return Err(format!("Expected '}}' at {}", self.cursor)),
                    _ => match self.parse_property() {
                        Ok(v) => tag.set(v.0, v.1),
                        Err(e) => return Err(e),
                    },
                };
            }

            _ => return Err(format!("Expected '{{' at {}", self.cursor)),
        }

        loop {
            match self.data.next() {
                Some(',') => match self.parse_property() {
                    Ok(v) => tag.set(v.0, v.1),
                    Err(e) => return Err(e),
                },

                Some('}') => return Ok(tag),
                _ => return Err(format!("Expected '}}' at {}", self.cursor)),
            }
        }
    }

    fn parse_property(&mut self) -> Result<(String, NbtTag), String> {
        self.handle_whitespace();
        match self.data.peek() {
            Some('a' ..= 'z') | Some('A' ..= 'Z') => match self.parse_string() {
                Ok(key) => {
                    self.handle_whitespace();
                    match self.data.next() {
                        Some(':') => {
                            self.handle_whitespace();
                            match self.parse_value() {
                                Ok(tag) => Ok((key, tag)),
                                Err(e) => Err(e),
                            }
                        }
                        _ => Err(format!("Expected : at {}", self.cursor)),
                    }
                }
                Err(e) => Err(e),
            },

            _ => return Err(format!("Expected '}}' at {}", self.cursor)),
        }
    }

    fn parse_value(&mut self) -> Result<NbtTag, String> {
        match self.data.peek() {
            Some('{') => match self.parse_compound_tag(NbtCompound::new()) {
                Ok(tag) => Ok(NbtTag::Compound(tag)),
                Err(e) => Err(e),
            },

            Some('0' ..= '9') | Some('.') => {
                let index = self.data.clone();
                match self.parse_num() {
                    Ok(tag) => return Ok(tag),
                    Err(_) => {
                        self.data = index;
                        match self.parse_string() {
                            Ok(string) => Ok(NbtTag::from(string)),
                            Err(e) => return Err(e),
                        }
                    }
                }
            }

            Some('\'') | Some('"') | Some('a' ..= 'z') | Some('A' ..= 'Z') =>
                match self.parse_string() {
                    Ok(str) => Ok(NbtTag::from(str)),
                    Err(e) => Err(e),
                },

            Some('[') => self.parse_list(),

            _ => Err(format!("Expected value at {}", self.cursor)),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        // Flag used to test what type of quotes to use with the string
        // 0 = no quotes
        // 1 = double quotes
        // 2 = single quotes
        let mut quotes = 0_u8;

        // Set the appropriate flag for the quotes found
        match self.data.peek() {
            Some('"') => {
                self.cursor += 1;
                self.data.next();
                quotes = 1;
            }
            Some('\'') => {
                self.cursor += 1;
                self.data.next();
                quotes = 2;
            }
            _ => {}
        };

        let mut output = String::new();

        loop {
            self.cursor += 1;
            match self.data.peek() {
                Some(c @ '}') | Some(c @ ':') | Some(c @ ',') => match quotes {
                    0 => break,
                    _ => {
                        output.push(*c);
                        self.data.next();
                    }
                },

                Some('"') => {
                    self.data.next();
                    match quotes {
                        2 => output.push('"'),
                        _ => break,
                    }
                }

                Some('\'') => {
                    self.data.next();
                    match quotes {
                        1 => output.push('\''),
                        _ => break,
                    }
                }

                Some('\\') => {
                    self.data.next();
                    self.cursor += 1;
                    match self.data.next() {
                        Some('\'') => output.push('\''),
                        Some('"') => output.push('"'),
                        Some('\\') => output.push('\\'),
                        Some(c @ _) =>
                            return Err(format!(
                                "Invalid escape character '\\{}'at {} ",
                                c, self.cursor
                            )),
                        None =>
                            return match quotes {
                                0 => break,
                                1 =>
                                    Err(format!("Unclosed double quote string at {}", self.cursor)),
                                2 =>
                                    Err(format!("Unclosed single quote string at {}", self.cursor)),
                                _ => unreachable!(),
                            },
                    }
                }

                Some(c @ 'a' ..= 'z')
                | Some(c @ 'A' ..= 'Z')
                | Some(c @ '0' ..= '9')
                | Some(c @ '+')
                | Some(c @ '-')
                | Some(c @ '_')
                | Some(c @ '.') => {
                    output.push(*c);
                    self.data.next();
                }

                Some(c @ _) => match quotes {
                    0 => break,
                    _ => {
                        output.push(*c);
                        self.data.next();
                    }
                },

                None => match quotes {
                    0 => break,
                    1 => return Err(format!("Unclosed double quote string at {}", self.cursor)),
                    2 => return Err(format!("Unclosed single quote string at {}", self.cursor)),
                    _ => unreachable!(),
                },
            };
        }
        Ok(output)
    }

    fn parse_num(&mut self) -> Result<NbtTag, String> {
        let mut num_string = String::new();
        let mut decimal = false;

        loop {
            self.cursor += 1;
            match self.data.peek() {
                Some(c @ '0' ..= '9') | Some(c @ '-') => {
                    num_string.push(*c);
                    self.data.next();
                }
                Some('.') => {
                    self.data.next();
                    if decimal {
                        return Err("Number has two decimal points".to_owned());
                    }
                    decimal = true;
                    num_string.push('.')
                }

                Some('b') | Some('B') => {
                    self.data.next();
                    if decimal {
                        return Err("Decimal in byte type".to_owned());
                    }
                    match self.data.peek() {
                        Some('}') | Some(']') | Some(',') => match num_string.parse::<i8>() {
                            Ok(val) => return Ok(NbtTag::Byte(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to a byte".to_owned()),
                        },

                        _ => return Err("Value is string, not number".to_owned()),
                    }
                }

                Some('l') | Some('L') => {
                    self.data.next();
                    if decimal {
                        return Err("Decimal in long type".to_owned());
                    }
                    match self.data.peek() {
                        Some('}') | Some(']') | Some(',') => match num_string.parse::<i64>() {
                            Ok(val) => return Ok(NbtTag::Long(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to a long".to_owned()),
                        },

                        _ => return Err("Value is string, not number".to_owned()),
                    }
                }

                Some('s') | Some('S') => {
                    self.data.next();
                    if decimal {
                        return Err("Decimal in short type".to_owned());
                    }
                    match self.data.peek() {
                        Some('}') | Some(']') | Some(',') => match num_string.parse::<i16>() {
                            Ok(val) => return Ok(NbtTag::Short(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to a short".to_owned()),
                        },

                        _ => return Err("Value is string, not number".to_owned()),
                    }
                }

                Some('f') | Some('F') => {
                    self.data.next();
                    if !decimal {
                        return Err("No decimal in float type".to_owned());
                    }
                    match self.data.peek() {
                        Some('}') | Some(']') | Some(',') => match num_string.parse::<f32>() {
                            Ok(val) => return Ok(NbtTag::Float(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to a float".to_owned()),
                        },

                        _ => return Err("Value is string, not number".to_owned()),
                    }
                }

                Some('d') | Some('D') => {
                    self.data.next();
                    if !decimal {
                        return Err("No decimal in double type".to_owned());
                    }
                    match self.data.peek() {
                        Some('}') | Some(']') | Some(',') => match num_string.parse::<f64>() {
                            Ok(val) => return Ok(NbtTag::Double(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to a double".to_owned()),
                        },

                        _ => return Err("Value is string, not number".to_owned()),
                    }
                }

                Some('}') | Some(']') | Some(',') | None =>
                    if decimal {
                        match num_string.parse::<f64>() {
                            Ok(val) => return Ok(NbtTag::Double(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to a double".to_owned()),
                        }
                    } else {
                        match num_string.parse::<i32>() {
                            Ok(val) => return Ok(NbtTag::Int(val)),
                            Err(_) =>
                                return Err("Couldn't parse number string to an int".to_owned()),
                        }
                    },

                Some(_) => return Err("Value is string, not number".to_owned()),
            }
        }
    }

    fn parse_list(&mut self) -> Result<NbtTag, String> {
        match self.data.next() {
            Some('[') => {}
            _ => return Err("List doesn't start with [".to_owned()),
        };

        self.handle_whitespace();

        match self.data.peek() {
            Some('I') | Some('B') | Some('L') => {
                let cache = self.data.clone();
                self.data.next();
                match self.data.next() {
                    Some(';') => {
                        self.data = cache;
                        return self.parse_array();
                    }
                    _ => self.data = cache,
                }
            }
            _ => {}
        }

        let mut output = NbtList::new();

        loop {
            self.handle_whitespace();
            match self.parse_value() {
                Ok(val) =>
                    if output.len() < 1 {
                        output.add(val)
                    } else {
                        if variant_eq(&val, output.get(0).unwrap()) {
                            output.add(val);
                        } else {
                            return Err(format!(
                                "Can't insert value of type {} into list of type {}",
                                val.type_string(),
                                output.get::<&NbtTag>(0).unwrap().type_string()
                            ));
                        }
                    },
                Err(_) => {}
            }

            self.handle_whitespace();

            match self.data.next() {
                Some(']') => return Ok(NbtTag::List(output)),
                Some(',') => {}
                Some(_) => return Err("Expected ,".to_owned()),
                None => return Err("Unclosed array".to_owned()),
            }
        }
    }

    fn parse_array(&mut self) -> Result<NbtTag, String> {
        self.handle_whitespace();
        match self.data.next() {
            Some('B') => parse_nbt_array!(Byte, i8, ByteArray, self),
            Some('I') => parse_nbt_array!(Int, i32, IntArray, self),
            Some('L') => parse_nbt_array!(Long, i64, LongArray, self),
            _ => Err("Not a typed array".to_owned()),
        }
    }

    fn handle_whitespace(&mut self) {
        loop {
            match self.data.peek() {
                Some(' ') => self.data.next(),
                _ => break,
            };
        }
    }
}

fn variant_eq<T>(a: &T, b: &T) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}
