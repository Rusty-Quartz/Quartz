use serde::{Serialize, Deserialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::env;
use std::fs;

pub fn gen_blockstates() {
    // set the output file
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("blockstate_output.rs");

    let mut output = String::new();

    // Load in the block info from blocks.json
    let mut data = serde_json::from_str::<HashMap<String, RawBlockInfo>>(include_str!("./assets/blocks.json")).expect("Error parsing blocks.json");
    
    // Find the shared properties
    let property_data = find_shared_properties(&data);
    output.push_str(&create_property_enums(&property_data));

    update_block_property_names(&mut data, &property_data);
    output.push_str(&gen_structs(&data));
    output.push_str(&gen_struct_enum(&data));


    fs::write(dest_path, output).unwrap();

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
            for i in 0..name_split.clone().count() {
                let mut second_word = name_split.next().unwrap().to_owned();
                second_word[..1].make_ascii_uppercase();
                if property_name == "has_book" {println!("{}{}", if i > 0 {"_"} else {""}, second_word)}
                cased_name.push_str(&format!("{}{}", if i > 0 {"_"} else {""}, second_word));
            }
            
            if possible_conflicts.contains_key(&cased_name) {
                possible_conflicts.get_mut(&cased_name).unwrap().push(block_name.clone());
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
            let block_properties = data.get(block).unwrap().properties.get(&lowercase_name).unwrap();
            let first_char = get_block_name(block).chars().nth(0).unwrap().to_ascii_uppercase();

            // If this is the first block in the property
            if property_values.is_empty() {
                property_values = block_properties.clone();
                enum_name.push(first_char);
                property_blocks.push(block.clone());
            } else {
                // If the property values match
                if vec_match(&property_values, block_properties) {
                    enum_name.push(first_char);
                    property_blocks.push(block.clone());
                } else {
                    match property_conflicts.get_mut(block_properties) {
                        // If an alt with the same properties already exists
                        Some((alt, block_vec)) => {
                            alt.push(first_char);
                            block_vec.push(block.clone());
                        }

                        None => {
                            property_conflicts.insert(block_properties.clone(), (format!("{}_{}", property_name, first_char), vec![block.clone()]));
                        }
                    }
                }
            }
        }

        // Insert the current property data in order to be able to just loop over property_conflicts
        property_conflicts.insert(property_values, (enum_name, property_blocks));

        for (values, (name, blocks)) in property_conflicts {
            let name = if values.get(0).unwrap().parse::<u8>().is_ok() {
                format!("{}_{}_{}", property_name, values.get(0).unwrap(), values.get(values.len()-1).unwrap())
            } else {
                name
            };
            property_data.push(PropertyData {name, blocks, values});
        }
    }

    property_data
}

fn create_property_enums(property_data: &Vec<PropertyData>) -> String {
    let mut enums  = String::new();
    for property in property_data.iter() {

        let is_num = property.values.get(0).unwrap().parse::<u8>().is_ok();
        let is_bool = property.values.get(0).unwrap().parse::<bool>().is_ok();

        let original_name = get_original_property_name(property);

        let mut curr_enum = format!("\n\n/// Blockstate property {} for", property.name);
        
        for block in &property.blocks {
            curr_enum.push_str(&format!("\n///\t{}", block));
        }
        
        if is_num {
            curr_enum.push_str("\n#[repr(u8)]");
        }
        curr_enum.push_str(&format!("\npub enum {} {{", property.name));

        let mut property_value_names = Vec::new();
        for value in &property.values {
            property_value_names.push(value.clone());
            if is_num {
                curr_enum.push_str(&format!("\n\t{}{} = {},", original_name, value.clone(), value.clone()))
            } else if is_bool {
                let mut var_name = value.clone();
                var_name[..1].make_ascii_uppercase();
                curr_enum.push_str(&format!("\n\t{},", var_name))
            } else {
                curr_enum.push_str(&format!("\n\t{},", value.clone()));
            }
        }

        enums.push_str(&format!("{}\n}}", curr_enum));
        
        let mut const_arr_name = property.name.clone();

        const_arr_name.make_ascii_uppercase();
        enums.push_str(&format!("\nconst {}_VALUES: [&str; {}] = [", const_arr_name, property_value_names.len()));
        
        for value in property_value_names {
            enums.push_str(&format!(r#""{}","#, value))
        }
        
        enums.push_str("];");
        
        enums.push_str(&format!("\nimpl {} {{\n\tpub const fn string_values() -> &'static [&'static str] {{\n\t\t", property.name.clone()));
        enums.push_str(&format!("&{}_VALUES", const_arr_name));
        enums.push_str("\n\t}\n}");
    }
    enums
}

fn update_block_property_names(block_data: &mut HashMap<String, RawBlockInfo>, property_data: &Vec<PropertyData>) {
    for property in property_data {

        // Calculate original property name in the json
        // I feel like this could be made less hacky but idk how
        let mut split_name = property.name.split('_');
        let mut lowercase_name = String::new();
        let offset = if property.values.get(0).unwrap().parse::<u8>().is_ok() {2} else {1};
        for i in 0..split_name.clone().count() - offset {
            lowercase_name.push_str(&format!("{}{}", if i > 0 {"_"} else {""}, split_name.next().unwrap()))
        }
        lowercase_name.make_ascii_lowercase();

        // replace the original name with the enum name
        for block in &property.blocks {
            let block_properties = block_data.get_mut(block).unwrap();
            println!("{} {} {:?}", lowercase_name, block, block_properties.properties);
            let vals = block_properties.properties.get(&lowercase_name).unwrap().clone();
            block_properties.properties.remove(&lowercase_name);
            block_properties.properties.insert(property.name.clone(), vals);
        }
    }
}

fn gen_structs(block_data: &HashMap<String, RawBlockInfo>) -> String {
    let mut output = String::new();

    for (uln_name, block_info) in block_data.iter() {
        let mut block_struct = String::new();

        let block_name = snake_to_camel(&get_block_name(uln_name));

        block_struct.push_str(&format!("\npub struct {}State {{", block_name));

        for (property_name, vals) in &block_info.properties {

            let lowercase_name = get_original_property_name(&PropertyData {name: property_name.to_owned(), values: vals.to_owned(), blocks: Vec::new()});

            block_struct.push_str(&format!("\n\t{}: {},", lowercase_name.replace("type", "r#type"), property_name));
        }
        block_struct.push_str("\n}");
        output.push_str(&block_struct);
    }

    output
}

fn gen_struct_enum(block_data: &HashMap<String, RawBlockInfo>) -> String {
    let mut enum_str = "\n#[repr(u16)]\npub enum BlockStateData {".to_owned();

    for (name, data) in block_data.iter() {
        let block_name = snake_to_camel(&get_block_name(name));
        enum_str.push_str(&format!("\n\t{}({}State) = {},", block_name, block_name, data.default))
    }
    
    enum_str.push_str("\n}");

    enum_str
}

fn vec_match(first: &Vec<String>, second: &Vec<String>) -> bool {
    if first.len() != second.len() {false}
    else {
        for i in 0..first.len() {
            if first.get(i) != second.get(i) {return false}
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
        word[..1].make_ascii_uppercase();
        output.push_str(&word);
    }
    output
}

fn get_original_property_name(property: &PropertyData) -> String {
    let mut split_name = property.name.split('_');
    let mut lowercase_name = String::new();
    let offset = if property.values.get(0).unwrap().parse::<u8>().is_ok() {2} else {1};
    for i in 0..split_name.clone().count() - offset {
        lowercase_name.push_str(&format!("{}{}", if i > 0 {"_"} else {""}, split_name.next().unwrap()))
    }
    lowercase_name.make_ascii_lowercase();
    lowercase_name
}

type StateID = u16;

#[derive(Serialize, Deserialize)]
struct RawBlockInfo {
    // Use a BTreeMap for ordering so that we can compute state IDs
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, Vec<String>>,
    default: StateID,
    states: Vec<RawStateInfo>
}

#[derive(Serialize, Deserialize)]
struct RawStateInfo {
    id: StateID,
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, String>
}

struct PropertyData {
    name: String,
    values: Vec<String>,
    blocks: Vec<String>
}