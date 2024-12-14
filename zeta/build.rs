use reqwest::blocking::get;
use std::{env, fs, path::Path};
use typify::{TypeSpace, TypeSpaceSettings};

fn main() {
    let schema = serde_json::from_str::<schemars::schema::RootSchema>(
        &get("https://zed.dev/schema/themes/v0.2.0.json")
            .unwrap()
            .text()
            .unwrap(),
    )
    .unwrap();

    let mut type_space = TypeSpace::new(TypeSpaceSettings::default().with_struct_builder(true));
    type_space.add_root_schema(schema).unwrap();

    let contents =
        prettyplease::unparse(&syn::parse2::<syn::File>(type_space.to_stream()).unwrap());

    let out_dir = env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join("schemas.rs"), contents).unwrap();
}
