use std::collections::HashMap;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub api_version: ApiVersion,
    pub methods: Vec<Method>,
    pub tg_categoryes: HashMap<String, String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiVersion {
    pub ver: String,
    pub date: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub names: (String, String, String),
    pub return_ty: Type,
    pub doc: Doc,
    pub tg_doc: String,
    pub tg_category: String,
    #[serde(default)]
    pub notes: Vec<Doc>,
    pub params: Vec<Param>,
    #[serde(default)]
    pub sibling: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doc {
    pub md: String,
    #[serde(default)]
    pub md_links: HashMap<String, String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub descr: Doc,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Type {
    True,
    u8,
    u16,
    u32,
    u64,
    i64,
    f64,
    bool,
    String,
    Option(Box<Type>),
    ArrayOf(Box<Type>),
    RawTy(String),
}

impl Schema {
    pub fn load(path: &str) -> Self {
        use std::io::Read;

        let mut file = std::fs::File::open(path).unwrap();
        let mut str = String::new();
        file.read_to_string(&mut str).unwrap();
        ron::from_str::<Schema>(&str).unwrap()
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::True => write!(f, "True"),
            Type::u8 => write!(f, "u8"),
            Type::u16 => write!(f, "u16"),
            Type::u32 => write!(f, "u32"),
            Type::u64 => write!(f, "u64"),
            Type::i64 => write!(f, "i64"),
            Type::f64 => write!(f, "f64"),
            Type::bool => write!(f, "bool"),
            Type::String => write!(f, "String"),
            Type::Option(inner) => write!(f, "Option<{}>", inner),
            Type::ArrayOf(inner) => write!(f, "Vec<{}>", inner),
            Type::RawTy(raw) => f.write_str(raw),
        }
    }
}
