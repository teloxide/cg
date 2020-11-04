use std::{borrow::Cow, collections::HashMap};

use once_cell::sync::Lazy;
use regex::Regex;

use crate::schema::Schema;

pub fn patch_sc(mut schema: Schema) -> Schema {
    fn check(l: &Option<&str>, r: &str) -> bool {
        l.map(|m| r == m).unwrap_or(true)
    }

    schema.methods.iter_mut().for_each(|method| {
        DOC_PATCHES
            .iter()
            .for_each(|(key, patch)| match key {
                Target::Method(m) => if check(m, &method.names.0) { 
                    method.doc.patch(patch, *key);
                },
                Target::Field { method_name: m, field_name: f } => if check(m, &method.names.0) {
                    method
                        .params
                        .iter_mut()
                        .filter(|p| check(f, &p.name))
                        .for_each(|p| p.descr.patch(patch, *key))
                },
                Target::Any { method_name: m } => if check(m, &method.names.0) {
                    method.doc.patch(patch, *key);

                    method
                        .params
                        .iter_mut()
                        .for_each(|p| p.descr.patch(patch, *key))
                },
            });
    });

    schema
}

static DOC_PATCHES: &[(Target, Patch)] = &[
    (Target::Any { method_name: None }, Patch::ReplaceLink { name: "More info on Sending Files »", value: "crate::types::InputFile" }),
    (Target::Any { method_name: None }, Patch::Custom(intra_links)),
    (Target::Method(Some("addStickerToSet")), Patch::Replace { text: "You **must** use exactly one of the fields _png\\_sticker_ or _tgs\\_sticker_. ", with: "" }),
    // FIXME RETUNRS
];

#[derive(Debug, Clone, Copy)]
enum Target<'a> {
    Any {
        method_name: Option<&'a str>,
    },
    Method(Option<&'a str>),
    Field {
        method_name: Option<&'a str>,
        field_name: Option<&'a str>,
    },
}

impl<'a> Target<'a> {
    fn method_name(&self) -> Option<&'a str> {
        *match self {
            Target::Any { method_name } => method_name,
            Target::Method(method_name) => method_name,
            Target::Field { method_name, field_name: _ } => method_name,
        }
    }

    fn is_exact(&self) -> bool {
        match self {
            Target::Method(m) => m.is_some(),
            Target::Field { method_name, field_name } => method_name.is_some() && field_name.is_some(),
            Target::Any { method_name } => false,
        }
    }
}

enum Patch<'a> {
    ReplaceLink {
        name: &'a str,
        value: &'a str,
    },
    AddLink {
        name: &'a str,
        value: &'a str,
    },
    RemoveLink {
        name: &'a str,
    },
    FullReplace {
        text: &'a str,
        with: &'a str,
    },
    Replace {
        text: &'a str,
        with: &'a str,
    },
    Custom(fn(&mut crate::schema::Doc))
}


impl crate::schema::Doc {
    fn patch(&mut self, patch: &Patch, key: Target) {
        match patch {
            Patch::ReplaceLink { name, value } => {
                if let Some(link) = self.md_links.get_mut(*name) {
                    link.clear();
                    *link += *value;
                } else if key.is_exact() {
                    panic!("Patch error: {:?} doesn't have link {}", key, name);
                }
            }
            Patch::AddLink { name, value } => {
                self.md_links.insert((*name).to_owned(), (*value).to_owned());
            }
            Patch::RemoveLink { name } => drop(self.md_links.remove(*name)),
            Patch::FullReplace { text, with } => {
                assert_eq!(self.md.as_str(), *text);

                self.md.clear();
                self.md += with;
            }
            Patch::Replace { text, with } => self.md = self.md.replace(*text, with),
            Patch::Custom(f) => f(self),
        } 
    }
}

fn intra_links(doc: &mut crate::schema::Doc) {
    let mut repls = Vec::new();
    doc
        .md_links
        .iter_mut()
        .filter(|(k, v)| v.starts_with("https://core.telegram.org/bots/api#") && !k.contains(&['-', '_', '.'][..]))
        .for_each(|(k, v)| if let Some(c) = k.chars().next() {
            repls.push(k.clone());
            kiam::when! {
                c.is_lowercase() => *v = format!("crate::payloads::{}", k),
                c.is_uppercase() => *v = format!("crate::types::{}", k),
            }
        });

    for repl in repls {
        if let Some(value) = doc.md_links.remove(repl.as_str()) {
            doc.md = doc.md.replace(format!("[{}]", repl).as_str(), &format!("[`{}`]", repl));
            doc.md_links.insert(format!("`{}`", repl), value);
        }
    }
}
