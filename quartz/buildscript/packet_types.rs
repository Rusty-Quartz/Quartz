use serde_json::from_str;
use serde::Deserialize;

use std::{collections::HashMap, env};
use std::path::Path;
use std::fs;

const SERDE_HEADER: &str = r#"
impl crate::network::PacketBuffer {
"#;

pub fn gen_packet_types() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("packet_types.rs");

    let types = from_str::<Vec<Type>>(include_str!("./assets/types.json")).unwrap();
    let mappings = from_str::<Mappings>(include_str!("./assets/mappings.json")).unwrap();

    let mut structs = String::new();
    let mut serde = SERDE_HEADER.to_owned();

    for custom_type in types {
        

        structs.push_str(&custom_type.gen_struct(&mappings.types));

        if custom_type.gen_serde {
            serde.push_str(&custom_type.gen_serializer(&mappings));
            serde.push_str(&custom_type.gen_deserializer(&mappings));
        }
    };

    serde.push_str("\n}");

    fs::write(dest_path, format!("{}\n\n{}", structs, serde)).unwrap();
}

#[derive(Deserialize)]
struct Type {
    name: String,
    fields: Vec<Field>,
    #[serde(default)]
    gen_serde: bool
}

impl Type {
    pub fn struct_name(&self) -> String {
        self.name.replace("_", "")
    }

    pub fn serde_name(&self) -> String {
        self.name.chars().fold(String::new(), |mut i, c| {
            if c.is_uppercase() {
                i.push_str("_");
                i.push(c);
                i
            } else {
                i.push(c);
                i
            }
        }).to_ascii_lowercase()
    }

    pub fn gen_struct(&self, type_maps: &HashMap<String, String>) -> String {
        let mut struct_str = format!("\npub struct {} {{", self.struct_name());

        for field in &self.fields {
            struct_str.push_str(&format!("\n\t{}: {}{}{}{}{},", 
                field.name,
                if field.option {"Option<"} else {""},
                if field.array {"Vec<"} else {""},
                parse_type(&field.var_type, &type_maps),
                if field.array {">"} else {""},
                if field.option {">"} else {""}
            ));
        }

        struct_str.push_str("\n}\n");
        struct_str
    }

    pub fn gen_serializer(&self, mappings: &Mappings) -> String {
        format!("\n\tpub fn write{}(&mut self, value: &{}) {{{}\n\t}}",
            self.serde_name(),
            self.struct_name(),
            self.fields.iter().fold(String::new(), |mut i, f| {
                i.push_str(&f.gen_serializer(mappings));
                i
            })
        )
    }

    pub fn gen_deserializer(&self, mappings: &Mappings) -> String {
        format!(
            "\n\tpub fn read{}(&mut self) -> {} {{{}\n\t\t{} {{{}\n\t\t}}\n\t}}",
            self.serde_name(),
            self.struct_name(),
            self.fields.iter().fold(String::new(), |mut i, f| {
                i.push_str(&f.gen_deserializer(mappings));
                i
            }),
            self.struct_name(),
            self.fields.iter().fold(String::new(), |mut i, f| {
                i.push_str(&format!("\n\t\t\t{},", f.name));
                i
            })
        )
    }
}

fn parse_type(field: &str, mappings: &HashMap<String, String>) -> String {
    let split = field.split("(").collect::<Vec<&str>>();
    let split = split.get(0).unwrap();

    if mappings.contains_key(split.to_owned()) {
        mappings.get(split.to_owned()).unwrap().to_owned()
    } else {
        split.to_owned().to_owned()
    }
}

#[derive(Deserialize)]
struct Field {
    name: String,
    #[serde(rename="type")]
    var_type: String,
    #[serde(default)]
    option: bool,
    #[serde(default)]
    array: bool,
    condition: Option<String>
}

impl Field {

    pub fn gen_deserializer(&self, mappings: &Mappings) -> String {
        if self.option {
            self.gen_option_deserializer(mappings)
        } else if self.array {
            format!("\n\t\tlet {} = {};",
                self.name,
                self.gen_array_deserializer(mappings)
            )
        } else {
            format!("\n\t\tlet {} = self.read_{}{};", 
                self.name, 
                self.var_type,
                if self.var_type.contains("(") {""} else {"()"}
            )
        }
    }

    pub fn gen_option_deserializer(&self, mappings: &Mappings) -> String {
        if self.array {
            format!("\n\t\tlet {} = if {} {{\n\t\t\tSome({})\n\t\t}} else {{None}};",
                self.name,
                self.condition.clone().unwrap(),
                self.gen_array_deserializer(mappings)
            )
        } else {
            format!("\n\t\tlet {} = if {} {{\n\t\t\tSome(self.read_{}{})\n\t\t}} else {{None}};",
                self.name,
                self.condition.clone().unwrap(),
                self.var_type,
                if self.var_type.contains("(") {""} else {"()"}
            )
        }
    }

    pub fn gen_array_deserializer(&self, mappings: &Mappings) -> String {
        format!("self.read_array({} as usize, PacketBuffer::read_{})",
            self.var_type.split("(").nth(1).unwrap().replace(")", ""),
            self.get_type()
        )
    }


    pub fn gen_serializer(&self, mappings: &Mappings) -> String {
        if self.option {
            self.gen_option_serializer(mappings)
        } else if self.array {
            format!("\n\t\t{}", self.gen_array_serializer(mappings))
        } else {
            format!("\n\t\tself.write_{}({}value.{});",
                self.get_type(),
                if mappings.primitives.contains(&self.get_type()) {""} else {"&"},
                self.name
            )
        }
    }

    pub fn gen_option_serializer(&self, mappings: &Mappings) -> String {
        if self.array {
            format!("\n\t\tmatch &value.{} {{\n\t\t\tSome(v) => {{{}}},\n\t\t\tNone => {{}}\n\t\t}}",
                self.name,
                self.gen_array_serializer(mappings)
            )
        } else {
            format!("\n\t\tmatch &value.{} {{\n\t\t\tSome(v) => self.write_{}({}v),\n\t\t\tNone => {{}}\n\t\t}}",
                self.name,
                self.get_type(),
                if mappings.primitives.contains(&self.get_type()) {"*"} else {""}
            )
        }
    }

    pub fn gen_array_serializer(&self, mappings: &Mappings) -> String {
        format!("self.write_{}array::<{}>(&value.{}, PacketBuffer::write_{});",
            if mappings.primitives.contains(&self.get_type()) {"primitive_"} else {""},
            parse_type(&self.var_type, &mappings.types),
            self.name,
            self.get_type()
        )
    }

    pub fn get_type(&self) -> String {
        self.var_type.split("(").next().unwrap().to_owned()
    }
}

#[derive(Deserialize)]
struct Mappings {
    types: HashMap<String, String>,
    primitives: Vec<String>
}