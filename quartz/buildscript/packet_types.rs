use serde::Deserialize;
use serde_json::from_str;

use std::{collections::HashMap, env, fs, path::Path};

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
            serde.push_str(&custom_type.gen_deserializer());
        }
    }

    serde.push_str("}");

    fs::write(&dest_path, format!("{}\n\n{}", structs, serde)).unwrap();
    super::format_in_place(dest_path.as_os_str());
}

#[derive(Deserialize)]
struct Type {
    name: String,
    fields: Vec<Field>,
    #[serde(default)]
    gen_serde: bool,
}

impl Type {
    pub fn struct_name(&self) -> String {
        self.name.replace("_", "")
    }

    pub fn serde_name(&self) -> String {
        self.name
            .chars()
            .fold(String::new(), |mut i, c| {
                if c.is_uppercase() {
                    i.push_str("_");
                    i.push(c);
                    i
                } else {
                    i.push(c);
                    i
                }
            })
            .to_ascii_lowercase()
    }

    pub fn gen_struct(&self, type_maps: &HashMap<String, String>) -> String {
        let mut struct_str = format!("pub struct {} {{", self.struct_name());

        for field in &self.fields {
            struct_str.push_str(&format!(
                "{}: {}{}{}{}{},",
                field.name,
                if field.option { "Option<" } else { "" },
                if field.array { "Vec<" } else { "" },
                parse_type(&field.var_type, &type_maps),
                if field.array { ">" } else { "" },
                if field.option { ">" } else { "" }
            ));
        }

        struct_str.push_str("}");
        struct_str
    }

    pub fn gen_serializer(&self, mappings: &Mappings) -> String {
        format!(
            "pub fn write{}(&mut self, value: &{}) {{{}}}",
            self.serde_name(),
            self.struct_name(),
            self.fields.iter().fold(String::new(), |mut i, f| {
                i.push_str(&f.gen_serializer(mappings));
                i
            })
        )
    }

    pub fn gen_deserializer(&self) -> String {
        format!(
            "pub fn read{}(&mut self) -> {} {{{}{} {{{}}}}}",
            self.serde_name(),
            self.struct_name(),
            self.fields.iter().fold(String::new(), |mut i, f| {
                i.push_str(&f.gen_deserializer());
                i
            }),
            self.struct_name(),
            self.fields.iter().fold(String::new(), |mut i, f| {
                i.push_str(&format!("{},", f.name));
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
    #[serde(rename = "type")]
    var_type: String,
    #[serde(default)]
    option: bool,
    #[serde(default)]
    array: bool,
    condition: Option<String>,
}

impl Field {
    pub fn gen_deserializer(&self) -> String {
        if self.option {
            self.gen_option_deserializer()
        } else if self.array {
            format!("let {} = {};", self.name, self.gen_array_deserializer())
        } else {
            format!(
                "let {} = self.read_{}{};",
                self.name,
                self.var_type,
                if self.var_type.contains("(") {
                    ""
                } else {
                    "()"
                }
            )
        }
    }

    pub fn gen_option_deserializer(&self) -> String {
        if self.array {
            format!(
                "let {} = if {} {{Some({})}} else {{None}};",
                self.name,
                self.condition.clone().unwrap(),
                self.gen_array_deserializer()
            )
        } else {
            format!(
                "let {} = if {} {{Some(self.read_{}{})}} else {{None}};",
                self.name,
                self.condition.clone().unwrap(),
                self.var_type,
                if self.var_type.contains("(") {
                    ""
                } else {
                    "()"
                }
            )
        }
    }

    pub fn gen_array_deserializer(&self) -> String {
        format!(
            "self.read_array({} as usize, PacketBuffer::read_{})",
            self.var_type.split("(").nth(1).unwrap().replace(")", ""),
            self.get_type()
        )
    }

    pub fn gen_serializer(&self, mappings: &Mappings) -> String {
        if self.option {
            self.gen_option_serializer(mappings)
        } else if self.array {
            format!("{}", self.gen_array_serializer(mappings))
        } else {
            format!(
                "self.write_{}({}value.{});",
                self.get_type(),
                if mappings.primitives.contains(&self.get_type()) {
                    ""
                } else {
                    "&"
                },
                self.name
            )
        }
    }

    pub fn gen_option_serializer(&self, mappings: &Mappings) -> String {
        if self.array {
            format!(
                "match &value.{} {{Some(v) => {{{}}},None => {{}}}}",
                self.name,
                self.gen_array_serializer(mappings)
            )
        } else {
            format!(
                "match &value.{} {{Some(v) => self.write_{}({}v),None => {{}}}}",
                self.name,
                self.get_type(),
                if mappings.primitives.contains(&self.get_type()) {
                    "*"
                } else {
                    ""
                }
            )
        }
    }

    pub fn gen_array_serializer(&self, mappings: &Mappings) -> String {
        format!(
            "self.write_{}array::<{}>(&value.{}, PacketBuffer::write_{});",
            if mappings.primitives.contains(&self.get_type()) {
                "primitive_"
            } else {
                ""
            },
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
    primitives: Vec<String>,
}
