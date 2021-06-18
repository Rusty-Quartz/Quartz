use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs,
    path::Path,
};

pub fn gen_blockstates() {
    // set the output file
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("blockstate_output.rs");

    // Load in the block info from blocks.json
    let mut data =
        serde_json::from_str::<HashMap<String, RawBlockInfo>>(include_str!("./assets/blocks.json"))
            .expect("Error parsing blocks.json");

    let mut output = r"use phf::{{phf_map}};".to_owned();
    // Find the shared properties
    let property_data = find_shared_properties(&data);
    output.push_str(&create_property_enums(&property_data));

    update_block_property_names(&mut data, &property_data);
    gen_default_states(&mut data, &property_data);
    output.push_str(&gen_structs(&data));
    output.push_str(&gen_struct_enum(&data));
    output.push_str(&gen_name_lookup(&data));

    fs::write(&dest_path, output).unwrap();
    super::format_in_place(dest_path.as_os_str());

    println!("cargo:rerun-if-changed=./assets/blocks.json");
    println!("cargo:rerun-if-changes=./blockstate.rs")
}

fn find_shared_properties(data: &HashMap<String, RawBlockInfo>) -> Vec<PropertyData> {
    let mut possible_conflicts: HashMap<String, Vec<String>> = HashMap::new();

    // Find all properties and find all blocks that share the same property name
    for (block_name, state_info) in data.iter() {
        for (property_name, _) in state_info.properties.iter() {
            let mut name_split = property_name.split('_');
            let mut cased_name = String::new();
            for i in 0 .. name_split.clone().count() {
                let mut second_word = name_split.next().unwrap().to_owned();
                second_word[.. 1].make_ascii_uppercase();
                if property_name == "has_book" {
                    println!("{}{}", if i > 0 { "_" } else { "" }, second_word)
                }
                cased_name.push_str(&format!("{}{}", if i > 0 { "_" } else { "" }, second_word));
            }

            if possible_conflicts.contains_key(&cased_name) {
                possible_conflicts
                    .get_mut(&cased_name)
                    .unwrap()
                    .push(block_name.clone());
            } else {
                possible_conflicts.insert(cased_name.clone(), vec![block_name.clone()]);
            }
        }
    }

    let mut property_data: Vec<PropertyData> = Vec::new();

    for (property_name, blocks) in possible_conflicts.iter() {
        let mut property_conflicts: HashMap<Vec<String>, (String, Vec<String>)> = HashMap::new();
        let mut lowercase_name = property_name.clone();
        lowercase_name.make_ascii_lowercase();
        let mut enum_name = property_name.clone();
        enum_name.push('_');
        let mut property_values: Vec<String> = Vec::new();
        let mut property_blocks: Vec<String> = Vec::new();

        for block in blocks {
            let block_properties = data
                .get(block)
                .unwrap()
                .properties
                .get(&lowercase_name)
                .unwrap();

            // If this is the first block in the property
            if property_values.is_empty() {
                property_values = block_properties.clone();
                property_blocks.push(block.clone());
            } else {
                // If the property values match
                if vec_match(&property_values, block_properties) {
                    property_blocks.push(block.clone());
                } else {
                    match property_conflicts.get_mut(block_properties) {
                        // If an alt with the same properties already exists
                        Some((_alt, block_vec)) => {
                            block_vec.push(block.clone());
                        }

                        None => {
                            let differences = get_differences(&property_values, block_properties);

                            let mut ending: String =
                                differences.iter().map(|v| v[.. 1].to_owned()).collect();
                            ending.make_ascii_uppercase();

                            if ending.len() == 0 {
                                let differences =
                                    get_differences(block_properties, &property_values);
                                let mut ending: String =
                                    differences.iter().map(|v| v[.. 1].to_owned()).collect();
                                ending.make_ascii_uppercase();
                                enum_name.push_str(&ending);
                            }

                            println!("{:?}  {}", differences, ending);

                            property_conflicts.insert(
                                block_properties.clone(),
                                (format!("{}_{}", property_name, ending), vec![block.clone()]),
                            );
                        }
                    }
                }
            }
        }

        // Insert the current property data in order to be able to just loop over property_conflicts
        property_conflicts.insert(property_values, (enum_name, property_blocks));

        for (values, (name, blocks)) in &property_conflicts {
            let name = if blocks.len() == 1 {
                format!(
                    "{}_{}",
                    property_name,
                    snake_to_camel(&get_block_name(&blocks.get(0).unwrap()))
                )
            } else if values.get(0).unwrap().parse::<u8>().is_ok() {
                format!(
                    "{}_{}",
                    property_name,
                    values.get(values.len() - 1).unwrap()
                )
            } else {
                name.clone()
            };
            property_data.push(PropertyData {
                name,
                blocks: blocks.clone(),
                values: values.clone(),
            });
        }
    }

    property_data
}

fn create_property_enums(property_data: &Vec<PropertyData>) -> String {
    let mut enums = String::new();
    for property in property_data.iter() {
        let is_num = property.values.get(0).unwrap().parse::<u8>().is_ok();
        let is_bool = property.values.get(0).unwrap().parse::<bool>().is_ok();
        println!("{}", property.values.get(0).unwrap());
        let original_name = get_original_property_name(property);
        let enum_name = if is_num {
            property.name.clone()
        } else {
            property.name.replace('_', "")
        };

        let mut curr_enum = format!("/// Blockstate property {} for", enum_name);

        for block in &property.blocks {
            curr_enum.push_str(&format!("\n/// {}", block));
        }

        curr_enum.push_str(&format!(
            "\n#[repr(u16)]#[derive(Clone, Copy, Debug)]pub enum {} {{",
            snake_to_camel(&enum_name)
        ));

        let mut property_value_names = Vec::new();
        for value in property.values.iter() {
            property_value_names.push(value.clone());
            if is_num {
                curr_enum.push_str(&format!(
                    "{}{},",
                    snake_to_camel(&original_name),
                    value.clone()
                ))
            } else if is_bool {
                let mut var_name = value.clone();
                var_name[.. 1].make_ascii_uppercase();
                curr_enum.push_str(&format!("{},", snake_to_camel(&var_name)))
            } else {
                curr_enum.push_str(&format!("{},", snake_to_camel(&value.clone())));
            }
        }

        enums.push_str(&format!("{}}}", curr_enum));

        let mut const_arr_name = enum_name.clone();

        const_arr_name.make_ascii_uppercase();
        enums.push_str(&format!(
            "const {}_VALUES: [&str; {}] = [",
            const_arr_name,
            property_value_names.len()
        ));

        for value in property_value_names.iter() {
            enums.push_str(&format!(r#""{}","#, value))
        }

        enums.push_str("];");

        enums.push_str(&format!(
            "impl {} {{pub const fn string_values() -> &'static [&'static str] {{",
            snake_to_camel(&enum_name)
        ));
        enums.push_str(&format!("&{}_VALUES}}", const_arr_name));

        enums.push_str(&format!(
            "const fn count() -> u16 {{{}}}",
            property_value_names.len()
        ));

        enums.push_str("fn from_str(s: &str) -> Option<Self> {match s {");
        for value in &property.values {
            let prop_value = if value.parse::<u8>().is_ok() {
                format!("{}{}", snake_to_camel(&original_name), value.clone())
            } else if value.parse::<bool>().is_ok() {
                let mut var_name = value.clone();
                var_name[.. 1].make_ascii_uppercase();
                snake_to_camel(&var_name)
            } else {
                snake_to_camel(&value.clone())
            };
            enums.push_str(&format!(
                "\"{}\" => Some({}::{}),",
                value,
                snake_to_camel(&enum_name),
                prop_value
            ))
        }
        enums.push_str("_ => None}}}");
    }
    enums
}

fn update_block_property_names(
    block_data: &mut HashMap<String, RawBlockInfo>,
    property_data: &Vec<PropertyData>,
) {
    for property in property_data {
        // replace the original name with the enum name
        let og_name = get_original_property_name(property);

        for block in &property.blocks {
            let block_properties = block_data.get_mut(block).unwrap();
            println!(
                "{} {} {} {:?}",
                property.name, og_name, block, block_properties.properties
            );
            let vals = block_properties.properties.get(&og_name).unwrap().clone();
            block_properties.properties.remove(&og_name);
            block_properties
                .properties
                .insert(property.name.clone(), vals);
        }
    }
}

fn gen_default_states(
    block_data: &mut HashMap<String, RawBlockInfo>,
    property_data: &Vec<PropertyData>,
) {
    for (block_name, block_info) in block_data.iter_mut() {
        let default_state_raw = block_info
            .states
            .iter()
            .find(|state| state.id == block_info.default)
            .unwrap()
            .properties
            .clone();
        let mut default_state = BTreeMap::new();

        for (prop_name, value) in default_state_raw {
            let property = property_data
                .iter()
                .find(|prop| {
                    get_original_property_name(prop) == prop_name
                        && prop.blocks.contains(block_name)
                })
                .unwrap();

            let prop_value = if value.parse::<u8>().is_ok() {
                format!("{}{}", snake_to_camel(&prop_name), value.clone())
            } else if value.parse::<bool>().is_ok() {
                let mut var_name = value.clone();
                var_name[.. 1].make_ascii_uppercase();
                snake_to_camel(&var_name)
            } else {
                snake_to_camel(&value.clone())
            };

            default_state.insert(
                prop_name.replace("type", "r#type"),
                format!("{}::{}", property.name.replace("_", ""), prop_value),
            );
        }

        block_info.default_state = default_state;
    }
}

fn gen_structs(block_data: &HashMap<String, RawBlockInfo>) -> String {
    let mut output = String::new();

    for (uln_name, block_info) in block_data.iter() {
        if block_info.properties.len() < 1 {
            continue;
        }

        let mut block_struct = String::new();
        let mut id_equation = String::new();
        let mut with_property = String::new();

        let block_name = snake_to_camel(&get_block_name(uln_name));

        block_struct.push_str(&format!(
            "#[derive(Clone, Copy, Debug)]pub struct {}State {{",
            snake_to_camel(&block_name)
        ));

        for (property_name, vals) in &block_info.properties {
            let lowercase_name = get_original_property_name(&PropertyData {
                name: property_name.to_owned(),
                values: vals.to_owned(),
                blocks: Vec::new(),
            });

            let field_name = lowercase_name.replace("type", "r#type");
            let type_name = snake_to_camel(&if vals.get(0).unwrap().parse::<u8>().is_ok() {
                property_name.clone()
            } else {
                property_name.replace('_', "")
            });

            block_struct.push_str(&format!("pub {}: {},", field_name, type_name));

            if id_equation.is_empty() {
                id_equation = format!("self.{} as u16", field_name);
            } else {
                id_equation = format!(
                    "({}) * {}::count() + self.{} as u16",
                    id_equation, type_name, field_name
                );
            }

            with_property.push_str(&format!(
                "\"{}\" => self.{} = {}::from_str(value)?,",
                lowercase_name, field_name, type_name
            ));
        }
        block_struct.push_str("}");

        // Default state value
        block_struct.push_str(&format!(
            "impl {0}State {{const fn const_default() -> Self {{{0}State ",
            snake_to_camel(&block_name)
        ));
        block_struct.push_str(
            &serde_json::to_string_pretty(&block_info.default_state)
                .unwrap()
                .replace("\"", ""),
        );
        block_struct.push_str("}");

        // ID computation
        block_struct.push_str(&format!(
            "pub const fn id(&self) -> u16 {{{} + {}}}",
            block_info.states[0].id, id_equation
        ));

        // Property value update
        block_struct.push_str(
            "pub(crate) fn with_property(mut self, name: &str, value: &str) -> Option<Self> { \
             match name {",
        );
        block_struct.push_str(&with_property);
        block_struct.push_str("_ => return None } Some(self) }");

        block_struct.push_str(&format!(
            "}}impl Default for {}State {{fn default() -> Self {{Self::const_default()}}}}",
            snake_to_camel(&block_name)
        ));

        output.push_str(&block_struct);
    }

    output
}

fn gen_struct_enum(block_data: &HashMap<String, RawBlockInfo>) -> String {
    let mut enum_str = "#[derive(Clone, Copy, Debug)]pub enum BlockStateData {".to_owned();

    let mut with_property = String::new();
    let mut id = String::new();

    for (name, data) in block_data.iter() {
        let block_name = snake_to_camel(&get_block_name(name));

        if data.properties.len() == 0 {
            enum_str.push_str(&format!("{},", block_name));
            with_property.push_str(&format!("Self::{} => None,", block_name));
            id.push_str(&format!("Self::{} => {},", block_name, data.states[0].id));
        } else {
            enum_str.push_str(&format!("{0}({0}State),", block_name));
            with_property.push_str(&format!(
                "Self::{0}(data) => Some(Self::{0}(data.with_property(name, value)?)),",
                block_name
            ));
            id.push_str(&format!("Self::{0}(data) => data.id(),", block_name));
        }
    }
    enum_str.push_str("}");

    enum_str.push_str("impl BlockStateData {");
    enum_str.push_str(
        "pub fn with_property(self, name: &str, value: &str) -> Option<Self> { match self {",
    );
    enum_str.push_str(&with_property);
    enum_str.push_str("} }");
    enum_str.push_str(&format!(
        "pub fn id(&self) -> u16 {{ match self {{{}}} }}",
        id
    ));
    enum_str.push_str("}");

    enum_str
}

fn gen_name_lookup(block_data: &HashMap<String, RawBlockInfo>) -> String {
    let mut lookup = String::new();

    lookup.push_str(
        "pub(crate) static BLOCK_LOOKUP_BY_NAME: phf::Map<&'static str, BlockStateMetadata> = \
         phf_map! {",
    );
    for (name, info) in block_data.iter() {
        let snake_name = get_block_name(name);
        let camel_name = snake_to_camel(&snake_name);

        let default_state = if info.properties.len() == 0 {
            format!("BlockStateData::{}", camel_name)
        } else {
            format!("BlockStateData::{}({0}State::const_default())", camel_name)
        };

        lookup.push_str(&format!(
            "\t\"{}\" => BlockStateMetadata::new({}, {}),\n",
            snake_name, default_state, info.interm_id
        ));
    }
    lookup.push_str("};");

    lookup
}

fn vec_match(first: &Vec<String>, second: &Vec<String>) -> bool {
    if first.len() != second.len() {
        false
    } else {
        for i in 0 .. first.len() {
            if first.get(i) != second.get(i) {
                return false;
            }
        }
        true
    }
}

fn get_block_name(uln: &str) -> String {
    let split: Vec<&str> = uln.split(':').collect();
    return (*split.get(1).unwrap()).to_owned();
}

fn snake_to_camel(str: &str) -> String {
    let split = str.split('_');
    let mut output = String::new();

    for part in split {
        let mut word = part.to_owned();
        println!("{}", word);
        if part == "" {
            continue;
        }
        word[.. 1].make_ascii_uppercase();
        output.push_str(&word);
    }
    output
}

fn get_original_property_name(property: &PropertyData) -> String {
    let mut split_name = property.name.split('_');
    let mut lowercase_name = String::new();
    let offset = 1; //if property.block_name {1} else { if property.values.get(0).unwrap().parse::<u8>().is_ok() {2} else {1} };
    println!(
        "{} {}",
        split_name.clone().collect::<String>(),
        property.name
    );
    for i in 0 .. split_name.clone().count() - offset {
        lowercase_name.push_str(&format!(
            "{}{}",
            if i > 0 { "_" } else { "" },
            split_name.next().unwrap()
        ))
    }
    lowercase_name.make_ascii_lowercase();
    lowercase_name
}

fn get_differences(first: &Vec<String>, second: &Vec<String>) -> Vec<String> {
    let mut differences = Vec::new();

    for val in second {
        if !first.contains(val) {
            differences.push(val.clone());
        }
    }

    differences
}

type StateID = u16;

#[derive(Serialize, Deserialize)]
struct RawBlockInfo {
    // Use a BTreeMap for ordering so that we can compute state IDs
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, Vec<String>>,
    default: StateID,
    interm_id: usize,
    #[serde(default = "BTreeMap::new")]
    default_state: BTreeMap<String, String>,
    states: Vec<RawStateInfo>,
}

#[derive(Serialize, Deserialize)]
struct RawStateInfo {
    id: StateID,
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, String>,
}

struct PropertyData {
    name: String,
    values: Vec<String>,
    blocks: Vec<String>,
}
