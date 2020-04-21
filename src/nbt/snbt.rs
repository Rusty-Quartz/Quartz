use crate::nbt::*;

use regex::Regex;

use log::*;

use lazy_static::lazy_static;

pub struct SnbtParser {
    data: String,
    cursor: usize
}

// All regexes are put in lazy_static! blocks so as to only compile the regex once

impl SnbtParser {

    pub fn new(data: &str) -> SnbtParser {
        SnbtParser {
            data: String::from(data), 
            cursor: 0
        }
    }
    
    // Parses SNBT to NBT
    pub fn parse(&mut self) -> Result<NbtCompound, String> {        
        let root_tag = NbtCompound::new();
        
        // Check to see if first character is { error otherwise
        match &self.data[self.cursor..self.cursor+1] {
            "{" => {
                self.cursor += 1;
                let output = self.parse_compound_tag(root_tag);

                // If we got an error immediatly return
                if output.is_err() {return output}

                // Error if we still hava data we didn't parse
                if self.cursor < self.data.len() -1 {
                    Err(format!("Unexpected characters after closing tag {}: {}", self.cursor, &self.data[self.cursor..self.data.len()]))
                }
                else {output}
            },
            _ => Err(format!("Invalid root tag: {}", &self.data[self.cursor..self.cursor+1]))
        }
    }
    
    fn parse_compound_tag(&mut self, mut tag: NbtCompound) -> Result<NbtCompound, String>{
        
        lazy_static! {
            static ref KEY_CHAR_REGEX: Regex = Regex::new(r"[a-zA-Z]").unwrap();
        }

        // Make sure we don't go out of bounds
        if self.cursor >= self.data.len()  {
            return Err(format!("NBT not closed"))
        }

        // Check to see if we have a key or just close the tag
        match &self.data[self.cursor..self.cursor+1] {
            char if KEY_CHAR_REGEX.is_match(char) => {
                
                // Parse the key
                let key = self.parse_key();
                if key.is_err() {return Err(key.unwrap_err())}

                // Parse the value
                let val = self.parse_value();
                if val.is_err() {return Err(val.err().unwrap())}

                // Put them in the compound tag
                tag.set(key.unwrap(), val.unwrap());

                // Check if we close the tag or keep parsing
                self.check_compound_tag_end(tag)
            },

            // Don't allow , right before }
            char if char == "}" && &self.data[self.cursor-1..self.cursor] != "," => {
                self.cursor += 1;
                Ok(tag)
            },

            _ => {
                Err(format!("Error parsing compound tag at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
            }
        }
    }

    fn check_compound_tag_end(&mut self, tag: NbtCompound) -> Result<NbtCompound, String>{
        // Make sure we don't go out of bounds
        if self.cursor >= self.data.len()  {
            return Err(format!("NBT not closed"))
        }

        // Check to see if we should continue parsing the tag or close it
        match &self.data[self.cursor..self.cursor+1] {

            "," => {
                // increment the cursor and check for more elements
                self.cursor += 1;
                self.parse_compound_tag(tag)
            },

            "}" => {
                // increment the cursor and close the tag
                self.cursor += 1;
                Ok(tag)
            },

            _ => {
                Err(format!("Error parsing compound tag ending at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
            }
        }
    }
    
    
    fn parse_key(&mut self) -> Result<String, String> {

        // Don't go out of bounds
        if self.cursor >= self.data.len() {
            return Err(format!("NBT not closed"))
        }
        
        lazy_static! {
            static ref KEY_PARSE_REGEX: Regex = Regex::new(r"[a-zA-Z-]+(?::)").unwrap();
        }
        
        // Get the key
        let capture = KEY_PARSE_REGEX.find(&self.data[self.cursor..]);

        // Check to make sure key is where the current index is and that it exists
        if capture.is_none() || capture.unwrap().start() != 0 {
            return Err(format!("Invalid property key at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
        }

        let capture = capture.unwrap();

        // Return the key and incrememnt the current index
        let output = Ok(self.data[self.cursor..capture.end()+self.cursor-1].to_owned());
        self.cursor += capture.end();
        output        
    }


    fn parse_value(&mut self) -> Result<NbtTag, String> {
        
        lazy_static!{
            static ref DIGIT: Regex = Regex::new(r"\d").unwrap();
            static ref STRING: Regex = Regex::new("[\"\']").unwrap();
        }
        
        // Don't go out of bounds
        if self.cursor >= self.data.len() {
            return Err(format!("NBT not closed"))
        }

        // Check to see what data type we're parsing
        match &self.data[self.cursor..self.cursor+1] {
            "{" => {

                // Increment off the { and parse the compound tag
                self.cursor += 1;
                let compound_tag = self.parse_compound_tag(NbtCompound::new());
                if compound_tag.is_err() {return Err(compound_tag.err().unwrap())}
                
                Ok(NbtTag::Compound(compound_tag.unwrap()))
            },

            char if DIGIT.is_match(char) => {

                // Parse any number type
                let num_tag = self.parse_num();
                if num_tag.is_err() {return Err(num_tag.err().unwrap())}
                
                Ok(num_tag.unwrap())
            },

            char if STRING.is_match(char) => {

                // increment off the " and parse the string
                self.cursor += 1;
                let string_tag = self.parse_string();
                if string_tag.is_err() {return Err(string_tag.err().unwrap())}

                Ok(string_tag.unwrap())
            },

            "[" => {

                // increment off the [ and parse the list
                self.cursor += 1;
                let list_tag = self.parse_list();
                if list_tag.is_err() {return Err(list_tag.err().unwrap())}

                Ok(NbtTag::List(list_tag.unwrap()))
            }
            _ => {
                Err(format!("Error parsing value at {}, {}", self.cursor, &self.data[self.cursor..self.data.len()]))
            }
        }
    }
    
    fn parse_num(&mut self) -> Result<NbtTag, String> {
        lazy_static! {
            static ref LIMITER: Regex = Regex::new(r"[\d\.]+\D").unwrap();
            static ref LONG: Regex = Regex::new(r"\d+(?:(l|L))").unwrap();
            static ref DOUBLE: Regex = Regex::new(r"(\d+(\.\d+)?)(?:(d|D))").unwrap();
            static ref BYTE: Regex = Regex::new(r"\d+(?:(b|B))").unwrap();
            static ref SHORT: Regex = Regex::new(r"\d+(?:(s|S))").unwrap();
            static ref FLOAT: Regex = Regex::new(r"(\d+(\.\d+)?)(?:(f|F))").unwrap();
            static ref INT: Regex = Regex::new(r"\d+").unwrap();
            static ref NOT_INT: Regex = Regex::new(r"[\d.]+(d|D|l|L|b|B|s|S|f|F)").unwrap(); // part of int test, makes sure there isn't a letter after the num
        }
        
        // Don't go out of bounds
        if self.cursor >= self.data.len()  {
            return Err(format!("NBT not closed"))
        }
        
        // Get how far ahead we should check to get the number's type
        let limit_capture = LIMITER.find(&self.data[self.cursor..]).unwrap();
        let limit = limit_capture.end() + self.cursor;
        
        // Check which number type we're parsing
        match &self.data[self.cursor..limit] {
            
            num if BYTE.is_match(num) => {
                let capture = BYTE.find(&self.data[self.cursor..limit]);
    
                // Make sure byte exists and is at the current index
                if capture.is_none() || capture.unwrap().start() != 0 {
                    return Err(format!("Invalid byte at {}, {}", &self.cursor, &self.data[self.cursor..limit]))
                }
                let capture = capture.unwrap();
    
                // Parse the byte from the string
                let parsed_byte = self.data[self.cursor..capture.end()+self.cursor-1].to_owned().parse::<i8>();
    
                // If there were errors parsing, error
                if parsed_byte.is_err() {
                    return Err(format!("Invalid value for type byte {}: {}, {}", self.cursor, &self.data[self.cursor..limit], parsed_byte.unwrap_err()))
                }
    
                // Return the byte and increase the current index
                let output = Ok(NbtTag::Byte(parsed_byte.unwrap()));
                self.cursor += capture.end();
                output
            },
            
            num if SHORT.is_match(num) => {
                let capture = SHORT.find(&self.data[self.cursor..limit]);

                // Make sure short exists and is at the current index
                if capture.is_none() || capture.unwrap().start() != 0 {
                    return Err(format!("Invalid short at {}, {}", &self.cursor, &self.data[self.cursor..limit]))
                }
                let capture = capture.unwrap();

                // Parse the short from the string
                let parsed_short = self.data[self.cursor..capture.end()+self.cursor-1].to_owned().parse::<i16>();

                // If there were errors parsing, error
                if parsed_short.is_err() {
                    return Err(format!("Invalid value for type short {}: {}, {}", self.cursor, &self.data[self.cursor..limit], parsed_short.unwrap_err()));
                }

                // Return the short and increase the current index
                let output = Ok(NbtTag::Short(parsed_short.unwrap()));
                self.cursor += capture.end();
                output
            },

            num if INT.is_match(num) && !NOT_INT.is_match(num) => {
                let capture = INT.find(&self.data[self.cursor..limit]);
    
                // Make sure int exists and is at the current index
                if capture.is_none() || capture.unwrap().start() != 0 {
                    return Err(format!("Invalid int at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
                }
                let capture = capture.unwrap();
    
                // Parse the int from the string
                let parsed_int = self.data[self.cursor..capture.end()+self.cursor].to_owned().parse::<i32>();
    
                // If there were errors parsing, error
                if parsed_int.is_err() {
                    return Err(format!("Invalid value for type int {}: {}, {}", self.cursor, &self.data[self.cursor..limit], parsed_int.unwrap_err()))
                }
    
                // Return the int and increase the current index
                let output = Ok(NbtTag::Int(parsed_int.unwrap()));
                self.cursor += capture.end();
                output
            },

            num if LONG.is_match(num) => {
                let capture = LONG.find(&self.data[self.cursor..limit]);

                // Make sure long exists and is at the current index
                if capture.is_none() || capture.unwrap().start() != 0 {
                    return Err(format!("Invalid long at {}, {}", &self.cursor, &self.data[self.cursor..limit]))
                }
                let capture = capture.unwrap();

                // Parse the long from the string
                let parsed_long = self.data[self.cursor..capture.end()+self.cursor-1].to_owned().parse::<i64>();

                // If there were errors parsing, error
                if parsed_long.is_err() {
                    return Err(format!("Invalid value for type long {}: {}, {}", self.cursor, &self.data[self.cursor..limit], parsed_long.unwrap_err()))
                }

                // Return the long and increase the current index
                let output = Ok(NbtTag::Long(parsed_long.unwrap()));
                self.cursor += capture.end();
                output
            },
            
            num if FLOAT.is_match(num) => {
                let capture = FLOAT.find(&self.data[self.cursor..limit]);

                // Make sure float exists and is at the current index
                if capture.is_none() || capture.unwrap().start() != 0 {
                    return Err(format!("Invalid float at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
                }
                let capture = capture.unwrap();

                // Parse the float from the string
                let parsed_float = self.data[self.cursor..capture.end()+self.cursor-1].to_owned().parse::<f32>();

                // If there were errors parsing, error
                if parsed_float.is_err() {
                    return Err(format!("Invalid value for type float {}: {}, {}", self.cursor, &self.data[self.cursor..limit], parsed_float.unwrap_err()))
                }

                // Return the float and increase the current index
                let output = Ok(NbtTag::Float(parsed_float.unwrap()));
                self.cursor += capture.end();
                output
            },

            num if DOUBLE.is_match(num) => {
                let capture = DOUBLE.find(&self.data[self.cursor..limit]);

                // Make sure double exists and is at the current index
                if capture.is_none() || capture.unwrap().start() != 0 {
                    return Err(format!("Invalid double at {}, {}", &self.cursor, &self.data[self.cursor..limit]))
                }
                let capture = capture.unwrap();

                // Parse the double from the string
                let parsed_double = self.data[self.cursor..capture.end()+self.cursor-1].to_owned().parse::<f64>();

                // If there were errors parsing, error
                if parsed_double.is_err() {
                    return Err(format!("Invalid value for type double {}: {}, {}", self.cursor, &self.data[self.cursor..limit], parsed_double.unwrap_err()))
                }

                // Return the double and increase the current index
                let output = Ok(NbtTag::Double(parsed_double.unwrap()));
                self.cursor += capture.end();
                output
            },

            _ => {
                Err(format!("Invalid number at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
            }
        }
    }

    fn parse_string(&mut self) -> Result<NbtTag, String>{
        
        lazy_static!{
            static ref STRING: Regex = Regex::new("(?:[^\"\\\\]|\\.)*(?:(\"|'))").unwrap();
        }

        // Don't go out of bounds
        if self.cursor >= self.data.len()  {
            return Err(format!("NBT not closed"))
        }

        // Find the string data
        let capture = STRING.find(&self.data[self.cursor..]);

        // Make sure its at the current index and exists
        if capture.is_none() || capture.unwrap().start() != 0 {
            return Err(format!("Invalid string at {}, {}", &self.cursor, &self.data[self.cursor..self.data.len()]))
        }
        let capture = capture.unwrap();

        // Write the string to an nbt tag and return it
        let output = Ok(NbtTag::StringModUtf8(self.data[self.cursor..capture.end()+self.cursor-1].to_owned()));
        self.cursor += capture.end();
        output
    }

    fn parse_list(&mut self) -> Result<NbtList, String> {
        // Create a new list tag and store the start index
        let mut list_tag = NbtList::new();
        let start_index = self.cursor;

        // loop until the list is closed
        loop {
            // Make sure we don't go out of bounds
            if self.cursor >= self.data.len() - 1 {
                return Err(format!("List not closed"))
            }

            // Parse the value
            let new_tag = self.parse_value();

            // Check for errors
            if new_tag.is_err() {return Err(new_tag.err().unwrap())}
            let new_tag = new_tag.unwrap();

            // Check if values are of same type
            if !SnbtParser::is_same_types(&list_tag, &new_tag) {
                return Err(format!("Values in list are not of same type, {}: {}", self.cursor, &self.data[start_index..self.cursor]))
            }

            // Add it to the list
            list_tag.add(new_tag);

            // check to see if we should close the list or if its malformed
            if &self.data[self.cursor..self.cursor+1] == "]" {self.cursor += 1; break}
            if &self.data[self.cursor..self.cursor+1] != "," {return Err(format!("Invalid list at {}, {}", &self.cursor, &self.data[start_index..self.cursor+1]))}
            self.cursor += 1;
        }

        // Return the list
        Ok(list_tag)
    }

    fn is_same_types(list: &NbtList, tag: &NbtTag) -> bool {
        if list.len() == 0{
            true
        }
        else if std::mem::discriminant(list.get(list.len()-1)) != std::mem::discriminant(&tag) {
           false
        }
        else {
            match tag {
                NbtTag::List(val) => {
                    match list.get(list.len()-1) {
                        NbtTag::List(val2) => {
                            SnbtParser::is_same_types(val2, val.get(val.len()-1))
                        }
                        _ => false
                    }
                },
                _ => true
            }
        }
    }
}