use std::{borrow::Borrow, collections::HashSet, ops::Deref};

use itertools::Itertools;
use kiam::when;

use crate::to_uppercase;

pub struct Payload {
    pub file_name: String,
    pub content: String,
}

impl Payload {
    pub fn generate(schema: &crate::schema::Schema) -> Vec<Self> {
        schema
            .methods
            .iter()
            .map(|method| {
                let file_name = [method.names.2.as_str(), ".rs"].concat();

                let uses = uses(&method);

                let method_doc = render_doc(&method.doc, method.sibling.as_deref());
                let eq_hash_derive = when! {
                    eq_hash_suitable(&method) => " Eq, Hash,",
                    _ => "",
                };
                let default_derive = when! {
                    default_needed(&method) => " Default,",
                    _ => "",
                };

                let return_ty = method.return_ty.to_string();

                let required = params(
                    method
                        .params
                        .iter()
                        .filter(|p| !matches!(&p.ty, crate::schema::Type::Option(_))),
                );
                let required = when! {
                    !required.is_empty() => format!("        required {{\n{}\n        }}", required),
                    _ => required,
                };

                let optional = params(
                    method
                        .params
                        .iter()
                        .filter_map(|p| match &p.ty {
                            crate::schema::Type::Option(inner) => {
                                Some(crate::schema::Param {
                                    name: p.name.clone(),
                                    ty: inner.deref().clone(),
                                    descr: p.descr.clone(),
                                })
                            },
                            _ => None,
                        })
                );
                let optional = when! {
                    !optional.is_empty() => format!("\n        optional {{\n{}\n        }}", optional),
                    _ => optional,
                };


                let multipart = multipart_input_file_fields(&method).map(|field| format!("    @[multipart = {}]\n", field.join(", "))).unwrap_or(String::new());

                let derive = if !multipart.is_empty() || ["SendMediaGroup", "EditMessageMedia", "EditMessageMediaInline"].contains(&&*method.names.1) {
                    format!("#[derive(Debug, Clone, Serialize)]")
                } else {
                    format!(
                        "#[derive(Debug, PartialEq,{eq_hash_derive}{default_derive} Clone, Serialize)]",
                        eq_hash_derive = eq_hash_derive,
                        default_derive = default_derive
                    )
                };

                let timeout_secs = when! {
                    method.names.2 == "get_updates" => "    @[timeout_secs = timeout]\n",
                    _ => ""
                };

                Payload {
                    file_name,
                    content: format!(
                        "\
{uses}

impl_payload! {{
{multipart}{timeout_secs}{method_doc}
    {derive}
    pub {Method} ({Method}Setters) => {return_ty} {{
{required}{optional}
    }}
}}
",
                        multipart = multipart,
                        timeout_secs = timeout_secs,
                        uses = uses,
                        method_doc = method_doc,
                        derive = derive,
                        Method = method.names.1,
                        return_ty = return_ty,
                        required = required,
                        optional = optional,
                    ),
                }
            })
            .collect()
    }
}

fn uses(method: &crate::schema::Method) -> String {
    enum Use {
        Prelude,
        Crate(String),
        External(String),
    }

    fn ty_use(ty: &crate::schema::Type) -> Use {
        match ty {
            crate::schema::Type::True => Use::Crate(String::from("use crate::types::True;")),
            crate::schema::Type::u8
            | crate::schema::Type::u16
            | crate::schema::Type::u32
            | crate::schema::Type::i32
            | crate::schema::Type::u64
            | crate::schema::Type::i64
            | crate::schema::Type::f64
            | crate::schema::Type::bool
            | crate::schema::Type::String => Use::Prelude,
            crate::schema::Type::Option(inner) | crate::schema::Type::ArrayOf(inner) => {
                ty_use(inner)
            }
            crate::schema::Type::RawTy(raw) => {
                Use::Crate(["use crate::types::", &raw, ";"].concat())
            }
            crate::schema::Type::Url => Use::External(String::from("use url::Url;")),
            crate::schema::Type::DateTime => {
                Use::External(String::from("use chrono::{DateTime, Utc};"))
            }
        }
    }

    let mut crate_uses = HashSet::new();
    let mut external_uses = HashSet::new();

    external_uses.insert(String::from("use serde::Serialize;"));

    core::iter::once(&method.return_ty)
        .chain(method.params.iter().map(|p| &p.ty))
        .map(ty_use)
        .for_each(|u| match u {
            Use::Prelude => {}
            Use::Crate(u) => {
                crate_uses.insert(u);
            }
            Use::External(u) => {
                external_uses.insert(u);
            }
        });

    let external_uses = external_uses.into_iter().join("\n");

    when! {
        crate_uses.is_empty() => external_uses,
        _ => {
            let crate_uses = crate_uses
                .into_iter()
                .join("");

            format!("{external}\n\n{crate_}", external = external_uses, crate_ = crate_uses)
        }
    }
}

fn render_doc(doc: &crate::schema::Doc, sibling: Option<&str>) -> String {
    let links = when! {
        doc.md_links.is_empty() => String::new(),
        _ => {
            let l = doc
                .md_links
                .iter()
                .map(|(name, link)| format!("[{}]: {}", name, link))
                .join("\n    /// ");

            ["\n    ///\n    /// ", &l].concat()
        }
    };

    let sibling_note = sibling
        .map(|s| {
            format!(
                "\n    /// \n    /// See also: [`{s}`](crate::payloads::{s})",
                s = to_uppercase(s)
            )
        })
        .unwrap_or_default();

    [
        "    /// ",
        &doc.md.replace("\n", "\n    /// "),
        &sibling_note,
        &links,
    ]
    .concat()
}

fn eq_hash_suitable(method: &crate::schema::Method) -> bool {
    fn ty_eq_hash_suitable(ty: &crate::schema::Type) -> bool {
        match ty {
            crate::schema::Type::f64 => false,
            crate::schema::Type::Option(inner) | crate::schema::Type::ArrayOf(inner) => {
                ty_eq_hash_suitable(&*inner)
            }

            crate::schema::Type::True
            | crate::schema::Type::u8
            | crate::schema::Type::u16
            | crate::schema::Type::u32
            | crate::schema::Type::i32
            | crate::schema::Type::u64
            | crate::schema::Type::i64
            | crate::schema::Type::bool
            | crate::schema::Type::String => true,

            crate::schema::Type::Url | crate::schema::Type::DateTime => true,

            crate::schema::Type::RawTy(raw) => raw != "MaskPosition" && raw != "InlineQueryResult",
        }
    }

    method.params.iter().all(|p| ty_eq_hash_suitable(&p.ty))
}

fn default_needed(method: &crate::schema::Method) -> bool {
    method
        .params
        .iter()
        .all(|p| matches!(p.ty, crate::schema::Type::Option(_)))
}

fn params(params: impl Iterator<Item = impl Borrow<crate::schema::Param>>) -> String {
    params
        .map(|param| {
            let param = param.borrow();
            let doc = render_doc(&param.descr, None).replace("\n", "\n        ");
            let field = &param.name;
            let ty = &param.ty;
            let flatten = match ty {
                crate::schema::Type::RawTy(s) if s == "InputSticker" || s == "TargetMessage" => {
                    "\n            #[serde(flatten)]"
                }
                _ => "",
            };
            let with = match ty {
                crate::schema::Type::DateTime => {
                    "\n            #[serde(with = \"crate::types::serde_opt_date_from_unix_timestamp\")]"
                }
                _ => "",
            };
            let rename = match field.strip_suffix('_') {
                Some(field) => format!("\n            #[serde(rename = \"{}\")]", field),
                None => "".to_owned(),
            };
            let convert = convert_for(ty);
            format!(
                "        {doc}{flatten}{with}{rename}\n            pub {field}: {ty}{convert},",
                doc = doc,
                flatten = flatten,
                with = with,
                rename = rename,
                field = field,
                ty = ty,
                convert = convert
            )
        })
        .join("\n")
}

pub(crate) fn convert_for(ty: &crate::schema::Type) -> Convert {
    match ty {
        ty @ crate::schema::Type::True
        | ty @ crate::schema::Type::u8
        | ty @ crate::schema::Type::u16
        | ty @ crate::schema::Type::u32
        | ty @ crate::schema::Type::i32
        | ty @ crate::schema::Type::u64
        | ty @ crate::schema::Type::i64
        | ty @ crate::schema::Type::f64
        | ty @ crate::schema::Type::bool => Convert::Id(ty.clone()),
        ty @ crate::schema::Type::String => Convert::Into(ty.clone()),
        crate::schema::Type::Option(inner) => convert_for(inner),
        crate::schema::Type::ArrayOf(ty) => Convert::Collect((**ty).clone()),
        crate::schema::Type::RawTy(s) => match s.as_str() {
            raw @ "Recipient" | raw @ "ChatId" | raw @ "TargetMessage" | raw @ "ReplyMarkup" => {
                Convert::Into(crate::schema::Type::RawTy(raw.to_owned()))
            }
            raw => Convert::Id(crate::schema::Type::RawTy(raw.to_owned())),
        },
        ty @ crate::schema::Type::Url => Convert::Id(ty.clone()),
        ty @ crate::schema::Type::DateTime => Convert::Into(ty.clone()),
    }
}

pub(crate) enum Convert {
    Id(crate::schema::Type),
    Into(crate::schema::Type),
    Collect(crate::schema::Type),
}

impl std::fmt::Display for Convert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Convert::Id(_) => Ok(()),
            Convert::Into(_) => f.write_str(" [into]"),
            Convert::Collect(_) => f.write_str(" [collect]"),
        }
    }
}

fn multipart_input_file_fields(m: &crate::schema::Method) -> Option<Vec<&str>> {
    let fields: Vec<_> = m
        .params
        .iter()
        .filter(|&p| ty_is_multiparty(&p.ty))
        .map(|p| &*p.name)
        .collect();

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

fn ty_is_multiparty(ty: &crate::schema::Type) -> bool {
    matches!(ty, crate::schema::Type::RawTy(x) if x == "InputFile" || x == "InputSticker")
        || matches!(ty, crate::schema::Type::Option(inner) if ty_is_multiparty(inner))
}
