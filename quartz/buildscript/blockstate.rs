use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs,
    path::Path,
    slice::Iter,
};
use syn::Expr;

pub fn gen_blockstates() {
    // set the output file
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("blockstate_output.rs");

    // Load in the block info from blocks.json
    let mut data =
        serde_json::from_str::<HashMap<String, RawBlockInfo>>(include_str!("./assets/blocks.json"))
            .expect("Error parsing blocks.json");

    // Find the shared properties
    let property_data = find_shared_properties(&data);
    let enums = create_property_enums(&property_data);

    update_block_property_names(&mut data, &property_data);
    gen_default_states(&mut data, &property_data);
    let structs = gen_structs(&data);
    let struct_enum = gen_struct_enum(&data);
    let lookup = gen_name_lookup(&data);

    fs::write(
        &dest_path,
        quote! {
            use phf::{phf_map};
            #enums
            #structs
            #struct_enum
            #lookup
        }
        .to_string(),
    )
    .unwrap();
    super::format_in_place(dest_path.as_os_str());

    println!("cargo:rerun-if-changed=buildscript/assets/blocks.json");
    println!("cargo:rerun-if-changes=buildscript/blockstate.rs")
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

fn create_property_enums(property_data: &Vec<PropertyData>) -> TokenStream {
    let enums = property_data.iter().map(|property| {
        let is_num = property.values.get(0).unwrap().parse::<u8>().is_ok();
        let original_name = get_original_property_name(property);
        let enum_name = if is_num {
            snake_to_camel(&property.name)
        } else {
            snake_to_camel(&property.name.replace('_', ""))
        };
        let enum_ident = format_ident!("{}", enum_name);

        let raw_property_value_names = property.values.clone();
        let property_value_names = property.values.iter().map(|v| {
            if is_num {
                format_ident!("{}{}", snake_to_camel(&original_name), v)
            } else {
                format_ident!("{}", snake_to_camel(v))
            }
        }).collect::<Vec<_>>();

        let arr_name = format_ident!("{}_VALUES", enum_name.to_ascii_uppercase());
        let arr_len = property_value_names.len();

        quote! {
            #[repr(u16)]
            #[derive(Clone, Copy, Debug)]
            pub enum #enum_ident {
                #(#property_value_names),*
            }

            const #arr_name: [&str; #arr_len] = [
                #(#raw_property_value_names),*
            ];

            impl #enum_ident {
                pub const fn string_values() -> &'static [&'static str] {
                    &#arr_name
                }

                pub const fn count() -> u16 {
                    #arr_len as u16
                }

                pub fn from_str(str: &str) -> Option<Self> {
                    match str {
                        #(#raw_property_value_names => Some(#enum_ident::#property_value_names), )*
                        _ => None
                    }
                }
            }
        }
    }).collect::<Vec<_>>();

    quote! {
        #(#enums)*
    }
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

            let vals = block_properties.properties.get(&og_name).unwrap().clone();
            block_properties.properties.remove(&og_name);
            block_properties.properties.insert(
                property.name.clone(), /*.replace("Type", "r#type")*/
                vals,
            );
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
                prop_name,
                format!("{}::{}", property.name.replace("_", ""), prop_value),
            );
        }

        block_info.default_state = default_state;
    }
}

fn gen_structs(block_data: &HashMap<String, RawBlockInfo>) -> TokenStream {
    let structs = block_data
        .iter()
        .filter(|(_uln_name, block_info)| {
            block_info.properties.len() != 0
        })
        .map(|(uln_name, block_info)| {
            let block_name = snake_to_camel(&get_block_name(uln_name));
            let block_state_name = format_ident!("{}State", block_name);
            let root_state = block_info.states[0].id;

            let mut vecs = (Vec::new(), Vec::new(), Vec::new());
            let (field_names, field_names_str, type_names) = block_info.properties.iter().fold(&mut vecs, |vecs, (property_name, values)| {
                let (field_names, field_names_str, type_names) = vecs;

                let lowercase_name = get_original_property_name(&PropertyData {
                    name: property_name.to_owned(),
                    values: values.to_owned(),
                    blocks: Vec::new(),
                });

                let field_name = lowercase_name;
                field_names.push(format_ident!("{}", field_name));
                field_names_str.push(field_name);
                type_names.push(format_ident!("{}", snake_to_camel(&if values.get(0).unwrap().parse::<u8>().is_ok() {
                    property_name.clone()
                } else {
                    property_name.replace('_', "")
                })));

                vecs
            });

            let id_eq = gen_id_eq(&mut type_names.iter().zip(field_names.iter()).collect::<Vec<_>>().iter());

            let default_vals = block_info.default_state.iter().map(|(_name, val)| {
                syn::parse_str::<Expr>(val).expect("How do we have an invalid expr")
            }).collect::<Vec<_>>();

            quote! {
                #[derive(Clone, Copy, Debug)]
                pub struct #block_state_name {
                    #(pub #field_names: #type_names),*
                }

                impl #block_state_name {
                    const fn const_default() -> Self {
                        #block_state_name {
                            #(#field_names: #default_vals),*
                        }
                    }

                    pub fn with_property(mut self, name: &str, value: &str) -> Option<Self> {
                        match name {
                            #(#field_names_str => self.#field_names = #type_names::from_str(value)?,)*
                            _ => return None
                        }

                        Some(self)
                    }

                    pub const fn id(&self) -> u16 {
                        #root_state + #id_eq
                    }
                }

                impl Default for #block_state_name {
                    fn default() -> Self {
                        Self::const_default()
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #(#structs)*
    }
}

fn gen_id_eq(states: &mut Iter<(&Ident, &Ident)>) -> Option<TokenStream> {
    let (type_name, field) = states.next()?;
    let last = gen_id_eq(states);

    Some(match last {
        Some(prev) => {
            quote! {
                (#prev) * #type_name::count() + self.#field as u16
            }
        }
        None => {
            quote! {
                self.#field as u16
            }
        }
    })
}

fn gen_struct_enum(block_data: &HashMap<String, RawBlockInfo>) -> TokenStream {
    let output = block_data
        .iter()
        .map(|(uln_name, block_data)| {
            let block_str = snake_to_camel(&get_block_name(uln_name));
            let block = format_ident!("{}", block_str);
            let block_state = format_ident!("{}State", block_str);

            if block_data.properties.len() == 0 {
                (
                    quote! {
                        Self::#block => None
                    },
                    quote! {
                        #block
                    },
                )
            } else {
                (
                    quote! {
                        Self::#block(data) => Some(Self::#block(data.with_property(name, value)?))
                    },
                    quote! {
                        #block(#block_state)
                    },
                )
            }
        })
        .unzip();

    let block_names: Vec<TokenStream> = output.1;
    let with_properties: Vec<TokenStream> = output.0;

    let ids = block_data
        .iter()
        .map(|(uln_name, block_data)| {
            let name = format_ident!("{}", snake_to_camel(&get_block_name(uln_name)));
            if block_data.properties.len() == 0 {
                let id = block_data.states[0].id;
                quote! {
                    Self::#name => #id
                }
            } else {
                quote! {
                    Self::#name(data) => data.id()
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #[derive(Clone, Copy, Debug)]
        pub enum BlockStateData {
            #(#block_names),*
        }

        impl BlockStateData {
            pub fn with_property(self, name: &str, value: &str) -> Option<Self> {
                match self {
                    #(#with_properties,)*
                    #[allow(unreachable_patterns)]
                    _ => None
                }
            }

            pub fn id(&self) -> u16 {
                match self {
                    #(#ids),*
                }
            }
        }
    }
}

fn gen_name_lookup(block_data: &HashMap<String, RawBlockInfo>) -> TokenStream {
    let lookups = block_data.iter().map(|(uln_name, block_data)| {
        let identifier = Literal::string(&uln_name[
            uln_name
                .char_indices()
                .find(|&(_, ch)| ch == ':')
                .map(|(index, _)| index + 1)
                .unwrap()
                ..
        ]);
        let block_str = snake_to_camel(&get_block_name(uln_name));
        let block = format_ident!("{}", block_str);
        let block_state = format_ident!("{}State", block_str);
        let internal_id = block_data.interm_id;

        if block_data.properties.len() == 0 {
            quote! {
                #identifier => BlockStateMetadata::new(BlockStateData::#block, #internal_id)
            }
        } else {
            quote! {
                #identifier => BlockStateMetadata::new(BlockStateData::#block(#block_state::const_default()), #internal_id)
            }
        }
    }).collect::<Vec<_>>();

    quote! {
        pub(crate) static BLOCK_LOOKUP_BY_NAME: phf::Map<&'static str, BlockStateMetadata> = phf_map! {
            #(#lookups),*
        };
    }
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
