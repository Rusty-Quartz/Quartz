use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    Data,
    DeriveInput,
    Error,
    Expr,
    Fields,
    GenericArgument,
    Ident,
    LitStr,
    PathArguments,
    Result,
    Token,
    Type,
};

pub fn parse_fields(input: &DeriveInput) -> Result<Vec<Field>> {
    let data_struct = match &input.data {
        Data::Struct(data_struct) => data_struct,
        _ => return Err(Error::new_spanned(&input.ident, "Expected struct")),
    };

    let named_fields = match &data_struct.fields {
        Fields::Named(named_fields) => named_fields,
        tokens @ _ => return Err(Error::new_spanned(tokens, "Struct fields must be named")),
    };

    let mut fields = Vec::new();
    for field_def in &named_fields.named {
        let attr = field_def
            .attrs
            .iter()
            .find(|&attr| attr.path.is_ident("packet_serde"));
        let params = attr
            .map(|attr| syn::parse2::<PacketSerdeParams>(attr.tokens.clone()))
            .transpose()?
            .unwrap_or_default();

        let ty = field_def.ty.clone();
        if is_vec(&ty) {
            let len = if params.greedy {
                let inner_type = extract_type_from_container(&ty)?;
                match inner_type {
                    Type::Path(path) =>
                        if path.qself.is_some() || !path.path.is_ident("u8") {
                            return Err(Error::new_spanned(
                                ty,
                                "Only Vec<u8> can be market as greedy",
                            ));
                        },
                    _ => return Err(Error::new_spanned(ty, "Expected path type")),
                }

                quote! { __buffer.remaining() }
            } else {
                if params.len.is_none() {
                    return Err(Error::new_spanned(ty, "Vecs must have a length expression"));
                }

                params.len.unwrap().to_token_stream()
            };

            fields.push(Field::array(
                field_def.ident.clone().unwrap(),
                ty,
                len,
                params.condition,
                false,
                params.varying,
            ));
            continue;
        }

        if params.greedy {
            return Err(Error::new_spanned(
                ty,
                "Only Vec<u8> can be market as greedy",
            ));
        }

        if is_option(&ty) {
            if params.condition.is_none() {
                return Err(Error::new_spanned(
                    ty,
                    "Options must have a condition expression",
                ));
            }

            let inner_ty = extract_type_from_container(&ty)?;
            if is_vec(&inner_ty) {
                if params.len.is_none() {
                    return Err(Error::new_spanned(
                        inner_ty,
                        "Vecs must have a length expression",
                    ));
                }

                fields.push(Field::array(
                    field_def.ident.clone().unwrap(),
                    inner_ty,
                    params.len.unwrap().to_token_stream(),
                    params.condition,
                    true,
                    params.varying,
                ));
            } else {
                if params.len.is_some() {
                    return Err(Error::new_spanned(
                        attr,
                        "Options cannot have a length expression",
                    ));
                }

                fields.push(Field::regular(
                    field_def.ident.clone().unwrap(),
                    inner_ty,
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
            field_def.ident.clone().unwrap(),
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

pub struct Field {
    pub name: Ident,
    pub ty: FieldType,
    pub condition: Option<Expr>,
    pub is_option: bool,
    pub varying: bool,
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
            ty: FieldType::Regular(ty),
            condition,
            is_option,
            varying,
        }
    }

    pub fn array(
        name: Ident,
        ty: Type,
        len: TokenStream,
        condition: Option<Expr>,
        is_option: bool,
        varying: bool,
    ) -> Self {
        Field {
            name,
            ty: FieldType::Array { ty, len },
            condition,
            is_option,
            varying,
        }
    }
}

pub enum FieldType {
    Regular(Type),
    Array { ty: Type, len: TokenStream },
}

fn is_vec(ty: &Type) -> bool {
    match ty {
        Type::Path(path) =>
            path.qself.is_none()
                && path.path.leading_colon.is_none()
                && !path.path.segments.is_empty()
                && path.path.segments.last().unwrap().ident == "Vec",
        _ => false,
    }
}

fn is_option(ty: &Type) -> bool {
    match ty {
        Type::Path(path) =>
            path.qself.is_none()
                && path.path.leading_colon.is_none()
                && !path.path.segments.is_empty()
                && path.path.segments.last().unwrap().ident == "Option",
        _ => false,
    }
}

fn extract_type_from_container(ty: &Type) -> Result<Type> {
    match ty {
        Type::Path(path) => {
            let type_params = &path.path.segments.last().unwrap().arguments;

            let generic_arg = match type_params {
                PathArguments::AngleBracketed(params) => params.args.first().unwrap(),
                tokens @ _ => return Err(Error::new_spanned(tokens, "Expected type parameter")),
            };

            match generic_arg {
                GenericArgument::Type(ty) => Ok(ty.clone()),
                arg @ _ => Err(Error::new_spanned(arg, "Expected type parameter")),
            }
        }
        ty @ _ => Err(Error::new_spanned(ty, "Expected path type")),
    }
}
