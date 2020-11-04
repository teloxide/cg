use std::{borrow::Borrow, ops::Deref, collections::HashSet, fmt::Display};

use itertools::Itertools;
use kiam::when;

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

                Payload {
                    file_name,
                    content: format!(
                        "\
{uses}

impl_payload! {{
{method_doc}
    #[derive(Debug, PartialEq,{eq_hash_derive} Clone, Deserialize, Serialize)]
    pub struct {Method} ({Method}Setters) => {return_ty} {{
{required}{optional}
    }}
}}
",
                        uses = uses,
                        method_doc = method_doc,
                        eq_hash_derive = eq_hash_derive,
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
    fn ty_use(ty: &crate::schema::Type) -> Option<String> {
        match ty {
            crate::schema::Type::True
            | crate::schema::Type::u8
            | crate::schema::Type::u16
            | crate::schema::Type::u32
            | crate::schema::Type::u64
            | crate::schema::Type::i64
            | crate::schema::Type::f64
            | crate::schema::Type::bool
            | crate::schema::Type::String => None,
            crate::schema::Type::Option(inner) | crate::schema::Type::ArrayOf(inner) => {
                ty_use(inner)
            }
            crate::schema::Type::RawTy(raw) => Some(["use crate::types::", &raw, ";\n"].concat()),
        }
    }

    let uses = core::iter::once(&method.return_ty)
        .chain(method.params.iter().map(|p| &p.ty))
        .flat_map(ty_use)
        .collect::<HashSet<_>>();

    when! {
        uses.is_empty() => String::from("use serde::{Serialize, Deserialize};"),
        _ => {
            let uses = uses
                .into_iter()
                .join("");

            format!("use serde::{{Serialize, Deserialize}};\n\n{uses}", uses = uses)
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

    let sibling_note = sibling.map(|s| format!("\n    /// \n    /// See also: [`{s}`](crate::payloads::{s})", s=s)).unwrap_or_default();

    ["    /// ", &doc.md.replace("\n", "\n    /// "), &sibling_note, &links].concat()
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
            | crate::schema::Type::u64
            | crate::schema::Type::i64
            | crate::schema::Type::bool
            | crate::schema::Type::String
            | crate::schema::Type::RawTy(_) => true,
        }
    }

    method.params.iter().all(|p| ty_eq_hash_suitable(&p.ty))
}

fn params(params: impl Iterator<Item = impl Borrow<crate::schema::Param>>) -> String {
    params
        .map(|param| {
            let param = param.borrow();
            let doc = render_doc(&param.descr, None).replace("\n", "\n        ");
            let field = &param.name;
            let ty = &param.ty;
            let flatten = match ty {
                crate::schema::Type::RawTy(s) if s == "InputSticker" || s == "TargetMessage" => "\n            #[serde(flatten)]",
                _ => "",
            };
            let convert = convert_for(ty);
            format!(
                "        {doc}{flatten}\n            pub {field}: {ty}{convert},",
                doc = doc,
                flatten = flatten,
                field = field,
                ty = ty,
                convert = convert
            )
        })
        .join("\n")
}

fn convert_for(ty: &crate::schema::Type) -> &'static str {
    match ty {
        crate::schema::Type::True
        | crate::schema::Type::u8
        | crate::schema::Type::u16
        | crate::schema::Type::u32
        | crate::schema::Type::u64
        | crate::schema::Type::i64
        | crate::schema::Type::f64
        | crate::schema::Type::bool => "",
        crate::schema::Type::String => " [into]",
        crate::schema::Type::Option(inner) => convert_for(inner),
        crate::schema::Type::ArrayOf(_) => " [collect]",
        crate::schema::Type::RawTy(s) => match s.as_str() {
            "ChatId" | "TargetMessage" | "ReplyMarkup" => " [into]",
            _ => "",
        },
    }
}
