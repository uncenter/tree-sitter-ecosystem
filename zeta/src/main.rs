use std::collections::HashMap;

use anyhow::Result;
use log::debug;
use tree_sitter::QueryCursor;
use zeta::{
    analyze,
    extensions::{ExtensionMetadata, ExtensionType},
};

include!(concat!(env!("OUT_DIR"), "/schemas.rs"));

fn main() -> Result<()> {
    env_logger::init();
    debug!("logger initialized");

    let extensions = analyze::collect_external_extensions()?;

    let mut total_extensions = 0;
    let mut toml_manifest_extensions = 0;
    let mut json_manifest_extensions = 0;
    let mut by_git_provider: HashMap<String, usize> = HashMap::new();
    let mut theme_extensions = 0;
    let mut language_extensions = 0;
    let mut unknown_extensions = 0;
    let mut supported_captures_by_theme: HashMap<String, Vec<String>> = HashMap::new();
    let mut captures_by_language: HashMap<String, Vec<String>> = HashMap::new();

    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_query::LANGUAGE.into())
        .expect("Error loading Query grammar");

    let query = tree_sitter::Query::new(
        &tree_sitter_query::LANGUAGE.into(),
        "(capture (identifier) @name) ",
    )
    .expect("tree-sitter-query capture query should build");

    for extension in extensions {
        total_extensions += 1;

        match extension.metadata {
            ExtensionMetadata::TomlManifest(_) => toml_manifest_extensions += 1,
            ExtensionMetadata::JsonManifest(_) => json_manifest_extensions += 1,
        }

        if !extension.builtin {
            *by_git_provider
                .entry(extension.git_provider.unwrap().clone())
                .or_default() += 1;
        }

        match extension.r#type {
            ExtensionType::Theme(theme_extension) => {
                theme_extensions += 1;
                let mut syntax_captures: Vec<String> = theme_extension
                    .themes
                    .iter()
                    .filter_map(|theme| {
                        theme.as_ref().map(|theme| {
                            theme
                                .themes
                                .iter()
                                .flat_map(|t| t.style.syntax.keys())
                                .collect::<Vec<&String>>()
                        })
                    })
                    .flatten()
                    .cloned()
                    .collect();

                syntax_captures.sort_unstable();
                syntax_captures.dedup();

                supported_captures_by_theme.insert(extension.id, syntax_captures);
            }
            ExtensionType::Language(language_extension) => {
                language_extensions += 1;
                let captures: Vec<String> = language_extension
                    .languages
                    .iter()
                    .filter_map(|language| match &language.highlights_queries {
                        Some(highlights) => {
                            let tree = parser.parse(highlights, None).unwrap();
                            let mut cursor = QueryCursor::new();
                            let captures =
                                cursor.captures(&query, tree.root_node(), highlights.as_bytes());

                            Some(vec![])
                        }
                        None => None,
                    })
                    .flatten()
                    .collect();

                captures_by_language.insert(extension.id, captures);
            }
            ExtensionType::Unknown => {
                unknown_extensions += 1;
            }
        }
    }

    println!("Total Extensions: {total_extensions}");
    println!(
        "\tWith TOML Manifest: {toml_manifest_extensions}\n\tWith JSON Manifest: {json_manifest_extensions}"
    );
    println!(
        "By Extension Type:\n\tTheme: {theme_extensions}\n\tLanguage: {language_extensions}\n\tUnknown: {unknown_extensions}"
    );
    println!("By Git Provider:");
    for (provider, count) in &by_git_provider {
        println!("\t{provider}: {count}");
    }

    Ok(())
}
