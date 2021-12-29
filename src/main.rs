use itertools::Itertools;

mod gen;
mod patch;
mod schema;

fn main() {
    let schema_path =
        std::env::var("SC_PATH").expect("Expected `SC_PATH` variable set (path to schema)");

    let schema = schema::Schema::load(&schema_path);
    let schema = patch::patch_sc(schema);
    let schema = patch::patch_ty(schema);

    let action = std::env::var("ACTION").expect("Expected `ACTION` variable set (action to do)");

    match action.as_str() {
        "0" => {
            let payloads_path = std::env::var("PL_PATH")
                .expect("Expected `PL_PATH` variable set (path to payloads)");

            payloads_main(schema, &payloads_path);
        }
        "1" => echo_payloads_modrs_and_settersrs_content(schema),
        "2" => echo_requester(schema),
        "3" => echo_requester_fwd_macro(schema),
        _ => unimplemented!("Unknown action"),
    }
}

fn payloads_main(schema: schema::Schema, payloads_path: &str) {
    use std::{fs::OpenOptions, io::Write, path::PathBuf};

    let header = header("file");

    let mut content = Vec::new();
    for payload in gen::Payload::generate(&schema) {
        let path = PathBuf::from(payloads_path).join(&payload.file_name);

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .expect(&format!("Failed to open {:?}", path));

        content.extend_from_slice(header.as_bytes());
        content.push(b'\n');
        content.extend_from_slice(payload.content.as_bytes());

        file.write_all(&content).expect("Write failed");

        content.clear();
    }
}

fn echo_payloads_modrs_and_settersrs_content(schema: schema::Schema) {
    println!("{}", header("block"));
    schema
        .methods
        .iter()
        .for_each(|m| println!("mod {};", m.names.2));
    println!();
    schema.methods.iter().for_each(|m| {
        println!(
            "pub use {m}::{{{M}, {M}Setters}};",
            m = m.names.2,
            M = m.names.1
        )
    });

    print!("\n\n\n");

    println!("{}", header("file"));

    println!("#[doc(no_inline)]\npub use crate::payloads::{{");
    schema
        .methods
        .iter()
        .for_each(|m| println!("    {M}Setters as _,", M = m.names.1));
    println!("}};");
}

fn echo_requester(schema: schema::Schema) {
    println!("{}", header("block"));
    schema.methods.iter().for_each(|m| {
        let mut convert_params = m
            .params
            .iter()
            .filter(|p| !matches!(p.ty, schema::Type::Option(_)))
            .map(|p| (&p.name, gen::convert_for(&p.ty)))
            .filter(|(_, c)| !matches!(c, gen::Convert::Id(_)))
            .collect::<Vec<_>>();

        convert_params.sort_by_key(|(name, _)| &**name);

        let mut prefixes = convert_params
            .iter()
            .tuple_windows()
            .map(|((l, _), (r, _))| (&***l, min_prefix(&l, &r).expect("No prefix...")))
            .collect::<indexmap::IndexMap<_, _>>();

        match convert_params.len() {
            0 => {}
            1 => drop(
                prefixes.insert(
                    &convert_params[0].0,
                    min_prefix(
                        &convert_params[0].0,
                        "\0", /* workaround to return &str*/
                    )
                    .expect("No prefix..."),
                ),
            ),
            n => drop(
                prefixes.insert(
                    &convert_params[n - 1].0,
                    min_prefix(&convert_params[n - 1].0, &convert_params[n - 2].0)
                        .expect("No prefix..."),
                ),
            ),
        }

        let args = m
            .params
            .iter()
            .filter(|p| !matches!(p.ty, schema::Type::Option(_)))
            .map(|p| match prefixes.get(&*p.name) {
                Some(prefix) => format!("{}: {}", p.name, to_uppercase(prefix)),
                None => format!("{}: {}", p.name, p.ty),
            })
            .join(", ");

        let generics = m
            .params
            .iter()
            .flat_map(|p| prefixes.get(&*p.name))
            .copied()
            .map(to_uppercase)
            .join(", ");
        let where_clause = m
            .params
            .iter()
            .filter(|p| !matches!(p.ty, schema::Type::Option(_)))
            .flat_map(|p| match gen::convert_for(&p.ty) {
                gen::Convert::Id(_) => None,
                gen::Convert::Into(ty) => Some(format!(
                    "{}: Into<{}>",
                    &to_uppercase(&prefixes[&*p.name]),
                    ty
                )),
                gen::Convert::Collect(ty) => Some(format!(
                    "{}: IntoIterator<Item = {}>",
                    &to_uppercase(&prefixes[&*p.name]),
                    ty
                )),
            })
            .join(",\n        ");

        let generics = kiam::when! {
            generics.is_empty() => String::from(""),
            _ => format!("<{}>", generics),
        };

        let args = kiam::when! {
            args.is_empty() => String::from(""),
            _ => format!(", {}", args),
        };

        let where_clause = kiam::when! {
            where_clause.is_empty() => String::from(""),
            _ => format!(" where {}", where_clause),
        };

        println!(
            "
    type {Method}: Request<Payload = {Method}, Err = Self::Err>;

    /// For Telegram documentation see [`{Method}`].
    fn {method} {generics} (&self{args}) -> Self::{Method}{where_clause};
            ",
            Method = m.names.1,
            method = m.names.2,
            args = args,
            generics = generics,
            where_clause = where_clause
        )
    });
}

fn echo_requester_fwd_macro(schema: schema::Schema) {
    println!("{}", header("macro"));
    println!(
        "macro_rules! requester_forward {{
    ($i:ident $(, $rest:ident )* $(,)? => $body:ident, $ty:ident ) => {{
        requester_forward!(@method $i $body $ty);
        $(
            requester_forward!(@method $rest $body $ty);
        )*
    }};"
    );
    schema.methods.iter().for_each(|m| {
        let mut convert_params = m
            .params
            .iter()
            .filter(|p| !matches!(p.ty, schema::Type::Option(_)))
            .map(|p| (&p.name, gen::convert_for(&p.ty)))
            .filter(|(_, c)| !matches!(c, gen::Convert::Id(_)))
            .collect::<Vec<_>>();

        convert_params.sort_by_key(|(name, _)| &**name);

        let mut prefixes = convert_params
            .iter()
            .tuple_windows()
            .map(|((l, _), (r, _))| (&***l, min_prefix(&l, &r).expect("No prefix...")))
            .collect::<indexmap::IndexMap<_, _>>();

        match convert_params.len() {
            0 => {}
            1 => drop(
                prefixes.insert(
                    &convert_params[0].0,
                    min_prefix(
                        &convert_params[0].0,
                        "\0", /* workaround to return &str*/
                    )
                    .expect("No prefix..."),
                ),
            ),
            n => drop(
                prefixes.insert(
                    &convert_params[n - 1].0,
                    min_prefix(&convert_params[n - 1].0, &convert_params[n - 2].0)
                        .expect("No prefix..."),
                ),
            ),
        }

        let args = m
            .params
            .iter()
            .filter(|p| !matches!(p.ty, schema::Type::Option(_)))
            .map(|p| match prefixes.get(&*p.name) {
                Some(prefix) => format!("{}: {}", p.name, to_uppercase(prefix)),
                None => format!("{}: {}", p.name, p.ty),
            })
            .join(", ");

        let generics = m
            .params
            .iter()
            .flat_map(|p| prefixes.get(&*p.name))
            .copied()
            .map(to_uppercase)
            .join(", ");
        let where_clause = m
            .params
            .iter()
            .filter(|p| !matches!(p.ty, schema::Type::Option(_)))
            .flat_map(|p| match gen::convert_for(&p.ty) {
                gen::Convert::Id(_) => None,
                gen::Convert::Into(ty) => Some(format!(
                    "{}: Into<{}>",
                    &to_uppercase(&prefixes[&*p.name]),
                    ty
                )),
                gen::Convert::Collect(ty) => Some(format!(
                    "{}: IntoIterator<Item = {}>",
                    &to_uppercase(&prefixes[&*p.name]),
                    ty
                )),
            })
            .join(",\n        ");

        let generics = kiam::when! {
            generics.is_empty() => String::from(""),
            _ => format!("<{}>", generics),
        };

        let before_args = kiam::when! {
            args.is_empty() => "",
            _ => ", ",
        };

        let where_clause = kiam::when! {
            where_clause.is_empty() => String::from(""),
            _ => format!(" where {}", where_clause),
        };

        println!(
            "

    (@method {method} $body:ident $ty:ident) => {{
        type {Method} = $ty![{Method}];

        fn {method}{generics}(&self{before_args}{args}) -> Self::{Method}{where_clause} {{
            let this = self;
            $body!({method} this ({args}))
        }}
    }};
    ",
            Method = m.names.1,
            method = m.names.2,
            before_args = before_args,
            args = args,
            generics = generics,
            where_clause = where_clause
        )
    });

    println!("}}");
}

fn to_uppercase(s: &str) -> String {
    let mut chars = s.chars();
    format!("{}{}", chars.next().unwrap().to_uppercase(), chars.as_str())
}

fn min_prefix<'a>(l: &'a str, r: &str) -> Option<&'a str> {
    l.char_indices()
        .zip(r.char_indices())
        .find(|((_, l), (_, r))| l != r)
        .map(|((i, _), (_, _))| &l[..=i])
}

fn header(thing: &str) -> String {
    format! {
    "\
// This {lower} is auto generated by [`cg`] from [`schema`].
//
// **DO NOT EDIT THIS {scream}**,
//
// Edit `cg` or `schema` instead.
// 
// [cg]: https://github.com/teloxide/cg
// [`schema`]: https://github.com/WaffleLapkin/tg-methods-schema",
         lower = thing,
         scream = thing.to_uppercase(),
    }
}
