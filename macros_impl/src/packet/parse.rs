use super::Side;
use crate::{extract_type_from_container, is_option, is_vec};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
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

    fn process_vec(
        fields: &mut Vec<Field>,
        name: Ident,
        ty: Type,
        params: PacketSerdeParams,
        is_option: bool,
        side: Side,
    ) -> Result<()> {
        let is_array_u8 = match extract_type_from_container(&ty)? {
            Type::Path(path) => path.qself.is_none() && path.path.is_ident("u8"),
            _ => return Err(Error::new_spanned(ty, "Expected path type")),
        };

        let len = if params.greedy {
            if !is_array_u8 {
                return Err(Error::new_spanned(
                    ty,
                    "Only Vec<u8> can be market as greedy",
                ));
            }

            quote! { __buffer.remaining() }
        } else {
            if side == Side::Read && params.len.is_none() {
                return Err(Error::new_spanned(ty, "Vecs must have a length expression"));
            }

            params.len.to_token_stream()
        };

        fields.push(Field::array(
            name,
            ty,
            len,
            params.condition,
            is_option,
            params.varying,
            is_array_u8,
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
        if is_vec(&ty) {
            process_vec(&mut fields, name, ty, params, false, side)?;
            continue;
        }

        if params.greedy {
            return Err(Error::new_spanned(
                ty,
                "Only Vec<u8> can be market as greedy",
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
            if is_vec(&inner_ty) {
                process_vec(&mut fields, name, inner_ty, params, true, side)?;
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
                ));
            }

            continue;
        }

        if params.len.is_some() {
            return Err(Error::new_spanned(
                attr,
                "Only Vecs (arrays) can have a length expression",
            ));
        }

        fields.push(Field::regular(
            name,
            ty,
            params.condition,
            false,
            params.varying,
        ));
    }

    Ok(fields)
}

struct PacketSerdeParams {
    varying: bool,
    greedy: bool,
    len: Option<Expr>,
    condition: Option<Expr>,
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

                    params.varying = true;
                }
                "greedy" => {
                    if params.greedy {
                        return Err(Error::new_spanned(ident, "Duplicate parameter"));
                    }

                    params.greedy = true;
                }
                "len" => {
                    if params.len.is_some() {
                        return Err(Error::new_spanned(ident, "Duplicate parameter"));
                    }

                    content.parse::<Token![=]>()?;
                    params.len = Some(syn::parse_str(&content.parse::<LitStr>()?.value())?);
                }
                "condition" => {
                    if params.condition.is_some() {
                        return Err(Error::new_spanned(ident, "Duplicate parameter"));
                    }

                    content.parse::<Token![=]>()?;
                    params.condition = Some(syn::parse_str(&content.parse::<LitStr>()?.value())?);
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
            len: None,
            condition: None,
        }
    }
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
    pub condition: Option<Expr>,
    pub is_option: bool,
    pub varying: bool,
    pub is_array_u8: bool,
}

impl Field {
    pub fn regular(
        name: Ident,
        ty: Type,
        condition: Option<Expr>,
        is_option: bool,
        varying: bool,
    ) -> Self {
        Field {
            name,
            raw_ty: ty,
            ty: FieldType::Regular,
            condition,
            is_option,
            varying,
            is_array_u8: false,
        }
    }

    pub fn array(
        name: Ident,
        ty: Type,
        len: TokenStream,
        condition: Option<Expr>,
        is_option: bool,
        varying: bool,
        is_array_u8: bool,
    ) -> Self {
        Field {
            name,
            raw_ty: ty,
            ty: FieldType::Array { len },
            condition,
            is_option,
            varying,
            is_array_u8,
        }
    }
}

pub enum FieldType {
    Regular,
    Array { len: TokenStream },
}
