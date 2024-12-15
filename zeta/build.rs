use reqwest::blocking::get;
use std::{env, fs, path::Path};
use typify::{TypeSpace, TypeSpaceSettings};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    for (url, name) in &[
        ("https://zed.dev/schema/themes/v0.1.0.json", "themes-v1"),
        ("https://zed.dev/schema/themes/v0.2.0.json", "themes-v2"),
    ] {
        let schema = get_schema(url);
        let rust = schema_to_rust(schema);
        fs::write(Path::new(&out_dir).join(format!("{name}.rs")), rust).unwrap();
    }
}

fn get_schema(url: &str) -> schemars::schema::RootSchema {
    serde_json::from_str::<schemars::schema::RootSchema>(&get(url).unwrap().text().unwrap())
        .unwrap()
}

fn schema_to_rust(schema: schemars::schema::RootSchema) -> String {
    let mut type_space = TypeSpace::new(TypeSpaceSettings::default().with_struct_builder(true));
    type_space.add_root_schema(schema).unwrap();
    type_space.to_stream().to_string()
}
