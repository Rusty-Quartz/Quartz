use super::Side;
use crate::{extract_type_from_container, is_boxed_slice, is_option};
use quote::format_ident;
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    Data,
    DataEnum,
    DeriveInput,
    Error,
    Expr,
    Fields,
    Ident,
    LitStr,
    Result,
    Token,
    Type,
};

pub(crate) fn parse_fields(input: &DeriveInput, side: Side) -> Result<Vec<Field>> {
    let data_struct = match &input.data {
        Data::Struct(data_struct) => data_struct,
        _ => return Err(Error::new_spanned(&input.ident, "Expected struct")),
    };

    let named_fields = match &data_struct.fields {
        Fields::Named(named_fields) => named_fields,
        tokens @ _ => return Err(Error::new_spanned(tokens, "Struct fields must be named")),
    };

    parse_fields_impl(&named_fields.named, true, side)
}

pub(crate) fn parse_enum(input: &DataEnum, side: Side) -> Result<Vec<EnumStructVariant>> {
    let mut variants = Vec::new();
    for variant in &input.variants {
        match &variant.fields {
            Fields::Named(named_fields) => {
                variants.push(EnumStructVariant {
                    name: variant.ident.clone(),
                    fields: parse_fields_impl(&named_fields.named, true, side)?,
                    is_tuple: false,
                });
            }
            Fields::Unnamed(unnamed_fields) => {
                variants.push(EnumStructVariant {
                    name: variant.ident.clone(),
                    fields: parse_fields_impl(&unnamed_fields.unnamed, false, side)?,
                    is_tuple: true,
                });
            }
            Fields::Unit => {
                variants.push(EnumStructVariant {
                    name: variant.ident.clone(),
                    fields: Vec::new(),
                    is_tuple: false,
                });
            }
        };
    }
    Ok(variants)
}

fn parse_fields_impl<'a, T>(
    field_defs: &'a T,
    require_names: bool,
    side: Side,
) -> Result<Vec<Field>>
where
    &'a T: IntoIterator<Item = &'a syn::Field>,
{
    let mut fields = Vec::new();

    fn process_array(
        fields: &mut Vec<Field>,
        name: Ident,
        boxed_slice_ty: Type,
        slice_ty: Type,
        params: PacketSerdeParams,
        is_option: bool,
        side: Side,
    ) -> Result<()> {
        let is_array_u8 = match extract_type_from_container(&slice_ty)? {
            Type::Path(path) =>
                path.qself.is_none()
                    && !path.path.segments.is_empty()
                    && path.path.segments.last().unwrap().ident == "u8",
            _ => return Err(Error::new_spanned(slice_ty, "Expected path type")),
        };

        if is_array_u8 && params.nbt {
            return Err(Error::new_spanned(
                boxed_slice_ty,
                "Type is not an NBT object",
            ));
        }

        let len = if params.greedy {
            // Ensures it's not NBT either
            if !is_array_u8 {
                return Err(Error::new_spanned(
                    boxed_slice_ty,
                    "Only Box<[u8]> can be market as greedy",
                ));
            }

            ArrayLength::Greedy
        } else {
            if side == Side::Read && params.len.is_none() {
                return Err(Error::new_spanned(
                    boxed_slice_ty,
                    "Arrays must have a length expression or be marked as `len_prefixed`",
                ));
            }

            params.len.unwrap()
        };

        fields.push(Field::array(
            name,
            boxed_slice_ty,
            len,
            params.condition,
            is_option,
            params.varying,
            is_array_u8,
            params.nbt,
        ));
        Ok(())
    }

    for (index, field_def) in field_defs.into_iter().enumerate() {
        let attr = field_def
            .attrs
            .iter()
            .find(|&attr| attr.path.is_ident("packet_serde"));
        let params = attr
            .map(|attr| syn::parse2::<PacketSerdeParams>(attr.tokens.clone()))
            .transpose()?
            .unwrap_or_default();

        let name = if require_names {
            match field_def.ident.clone() {
                Some(ident) => ident,
                None => return Err(Error::new_spanned(field_def, "Fields must be named")),
            }
        } else {
            match &field_def.ident {
                Some(ident) => ident.clone(),
                None => format_ident!("var{}", index),
            }
        };

        let ty = field_def.ty.clone();
        if is_boxed_slice(&ty) {
            // Unwrap guaranteed by is_boxed_slice
            let slice_ty = extract_type_from_container(&ty).unwrap();
            process_array(&mut fields, name, ty, slice_ty, params, false, side)?;
            continue;
        }

        if params.greedy {
            return Err(Error::new_spanned(
                ty,
                "Only Box<[u8]> can be market as greedy",
            ));
        }

        if is_option(&ty) {
            if side == Side::Read && params.condition.is_none() {
                return Err(Error::new_spanned(
                    ty,
                    "Options must have a condition expression",
                ));
            }

            let inner_ty = extract_type_from_container(&ty)?;
            if is_boxed_slice(&inner_ty) {
                // Unwrap guaranteed by is_boxed_slice
                let slice_ty = extract_type_from_container(&inner_ty).unwrap();
                process_array(&mut fields, name, inner_ty, slice_ty, params, true, side)?;
            } else {
                if params.len.is_some() {
                    return Err(Error::new_spanned(
                        attr,
                        "Options cannot have a length expression",
                    ));
                }

                fields.push(Field::regular(
                    name,
                    ty,
                    params.condition,
                    true,
                    params.varying,
                    params.nbt,
                ));
            }

            continue;
        }

        if params.len.is_some() {
            return Err(Error::new_spanned(
                attr,
                "Only boxed slices (arrays) can have a length",
            ));
        }

        if params.condition.is_some() {
            return Err(Error::new_spanned(
                attr,
                "Only boxed options can have a condition",
            ));
        }

        fields.push(Field::regular(
            name,
            ty,
            params.condition,
            false,
            params.varying,
            params.nbt,
        ));
    }

    Ok(fields)
}

struct PacketSerdeParams {
    varying: bool,
    greedy: bool,
    nbt: bool,
    len: Option<ArrayLength>,
    condition: Option<OptionCondition>,
}

impl Parse for PacketSerdeParams {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut params = Self::default();
        let content;
        parenthesized!(content in input);

        while !content.is_empty() {
            let ident: Ident = content.parse()?;
            match ident.to_string().as_str() {
                "varying" => {
                    if params.varying {
                        return Err(Error::new_spanned(ident, "Duplicate parameter"));
                    }

                    if params.nbt {
                        return Err(Error::new_spanned(
                            ident,
                            "Parameter incompatible with `nbt`",
                        ));
                    }

                    params.varying = true;
                }
                "greedy" => {
                    if params.greedy {
                        return Err(Error::new_spanned(ident, "Duplicate parameter"));
                    }

                    params.greedy = true;
                }
                "nbt" => {
                    if params.nbt {
                        return Err(Error::new_spanned(ident, "Duplicate parameter"));
                    }

                    if params.greedy {
                        return Err(Error::new_spanned(
                            ident,
                            "Parameter incompatible with `varying`",
                        ));
                    }

                    params.nbt = true;
                }
                "len" => {
                    if params.len.is_some() {
                        return Err(Error::new_spanned(ident, "Duplicate length parameter"));
                    }

                    content.parse::<Token![=]>()?;
                    params.len = Some(ArrayLength::Expr(syn::parse_str(
                        &content.parse::<LitStr>()?.value(),
                    )?));
                }
                "len_prefixed" => {
                    if params.len.is_some() {
                        return Err(Error::new_spanned(ident, "Duplicate length parameter"));
                    }

                    params.len = Some(ArrayLength::Prefixed);
                }
                "condition" => {
                    if params.condition.is_some() {
                        return Err(Error::new_spanned(ident, "Duplicate condition parameter"));
                    }

                    content.parse::<Token![=]>()?;
                    params.condition = Some(OptionCondition::Expr(syn::parse_str(
                        &content.parse::<LitStr>()?.value(),
                    )?));
                }
                "bool_prefixed" => {
                    if params.condition.is_some() {
                        return Err(Error::new_spanned(ident, "Duplicate condition parameter"));
                    }

                    params.condition = Some(OptionCondition::Prefixed);
                }
                _ =>
                    return Err(Error::new_spanned(
                        ident,
                        "Unknown parameter, expected one of `varying`, `greedy`, `len`, or \
                         `condition`",
                    )),
            }

            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(params)
    }
}

impl Default for PacketSerdeParams {
    fn default() -> Self {
        PacketSerdeParams {
            varying: false,
            greedy: false,
            nbt: false,
            len: None,
            condition: None,
        }
    }
}

pub enum ArrayLength {
    Expr(Expr),
    Prefixed,
    Greedy,
}

pub enum OptionCondition {
    Expr(Expr),
    Prefixed,
}

pub struct EnumStructVariant {
    pub name: Ident,
    pub fields: Vec<Field>,
    pub is_tuple: bool,
}

pub struct Field {
    pub name: Ident,
    pub raw_ty: Type,
    pub ty: FieldType,
    pub condition: Option<OptionCondition>,
    pub is_option: bool,
    pub varying: bool,
    pub is_array_u8: bool,
    pub is_nbt: bool,
}

impl Field {
    pub fn regular(
        name: Ident,
        ty: Type,
        condition: Option<OptionCondition>,
        is_option: bool,
        varying: bool,
        is_nbt: bool,
    ) -> Self {
        Field {
            name,
            raw_ty: ty,
            ty: FieldType::Regular,
            condition,
            is_option,
            varying,
            is_array_u8: false,
            is_nbt,
        }
    }

    pub fn array(
        name: Ident,
        ty: Type,
        len: ArrayLength,
        condition: Option<OptionCondition>,
        is_option: bool,
        varying: bool,
        is_array_u8: bool,
        is_nbt: bool,
    ) -> Self {
        assert!(
            !(is_array_u8 && is_nbt),
            "An array field cannot both be a byte buffer and an NBT object"
        );

        Field {
            name,
            raw_ty: ty,
            ty: FieldType::Array { len },
            condition,
            is_option,
            varying,
            is_array_u8,
            is_nbt,
        }
    }
}

pub enum FieldType {
    Regular,
    Array { len: ArrayLength },
}
