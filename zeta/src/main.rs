use anyhow::Result;
use clap::{arg, Parser, Subcommand, ValueEnum};
use log::debug;
use std::{collections::HashMap, fs};

use streaming_iterator::StreamingIterator;
use tree_sitter::QueryCursor;

use zeta::{
    extensions::{Extension, ExtensionMetadata, ExtensionType},
    scan,
};

#[derive(Parser)]
#[command(version, about, arg_required_else_help(true))]
struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long)]
    pub refresh: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    Compare {
        comparison: Comparisons,
    },
    Query {
        #[command(subcommand)]
        query: Queries,
    },
}

#[derive(Clone, ValueEnum)]
pub enum Comparisons {
    ByType,
    ByManifest,
    ByGitProvider,
}

#[derive(Subcommand)]
pub enum Queries {
    ThemesSupportingCapture { capture: String },
    MostCommonlyUsedCaptures,
    LeastCommonlyUsedCaptures,
    MostSupportedCaptures,
    LeastSupportedCaptures,
    LanguageCapturesSupportByTheme { theme: String },
}

fn main() -> Result<()> {
    env_logger::init();
    debug!("logger initialized");

    let args: Cli = Cli::parse();

    let cache_dir = user_dirs::cache_dir()?.join("ts-ecosystem-zeta");
    let extensions_scan_cache = cache_dir.join("extensions-scan-dump.json");

    let (extensions, cache_hit): (Vec<Extension>, bool) = if args.refresh {
        (
            scan::scan_extensions(cache_dir.join("extensions-clone"))?,
            false,
        )
    } else {
        match fs::read_to_string(&extensions_scan_cache) {
            Ok(contents) => (serde_json::from_str(&contents)?, true),
            Err(_) => (
                scan::scan_extensions(cache_dir.join("extensions-clone"))?,
                false,
            ),
        }
    };

    if !cache_hit {
        fs::write(&extensions_scan_cache, serde_json::to_string(&extensions)?)?;
    }

    match args.command {
        Commands::Compare { comparison } => match comparison {
            Comparisons::ByType => {
                let mut language_extension_count = 0;
                let mut theme_extension_count = 0;
                let mut unknown_extension_count = 0;

                for extension in extensions {
                    match extension.r#type {
                        ExtensionType::Theme(_) => theme_extension_count += 1,
                        ExtensionType::Language(_) => language_extension_count += 1,
                        ExtensionType::Unknown => unknown_extension_count += 1,
                    }
                }

                println!(
                    "By Extension Type:\n\tTheme: {theme_extension_count}\n\tLanguage: {language_extension_count}\n\tUnknown: {unknown_extension_count}"
                );
            }
            Comparisons::ByManifest => {
                let mut toml_manifest_count = 0;
                let mut json_manifest_count = 0;

                for extension in extensions {
                    match extension.metadata {
                        ExtensionMetadata::TomlManifest(_) => toml_manifest_count += 1,
                        ExtensionMetadata::JsonManifest(_) => json_manifest_count += 1,
                    }
                }

                println!("By Manifest Type:\n\tWith TOML Manifest: {toml_manifest_count}\n\tWith JSON Manifest: {json_manifest_count}");
            }
            Comparisons::ByGitProvider => {
                let mut by_git_provider: HashMap<String, usize> = HashMap::new();

                for extension in extensions {
                    if !extension.builtin {
                        *by_git_provider
                            .entry(
                                extension
                                    .git_provider
                                    .expect("non-builtin extensions should have a git_provider")
                                    .clone(),
                            )
                            .or_default() += 1;
                    }
                }

                println!("By Git Provider:");
                for (provider, count) in &by_git_provider {
                    println!("\t{provider}: {count}");
                }
            }
        },
        Commands::Query { query } => {
            let mut supported_captures_by_theme: HashMap<String, Vec<String>> = HashMap::new();
            let mut captures_by_language: HashMap<String, Vec<String>> = HashMap::new();

            let mut ts_parser = tree_sitter::Parser::new();
            ts_parser
                .set_language(&tree_sitter_query::LANGUAGE.into())
                .expect("Error loading Query grammar");

            let ts_query = tree_sitter::Query::new(
                &tree_sitter_query::LANGUAGE.into(),
                "(capture (identifier) @name) ",
            )
            .expect("tree-sitter-query capture query should build");

            for extension in extensions {
                match extension.r#type {
                    ExtensionType::Theme(theme_extension) => {
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
                        let captures: Vec<String> = language_extension
                            .languages
                            .iter()
                            .filter_map(|language| match &language.highlights_queries {
                                Some(highlights) => {
                                    let tree = ts_parser.parse(highlights, None).unwrap();
                                    let text = highlights.as_bytes();
                                    let mut cursor = QueryCursor::new();
                                    let mut captures =
                                        cursor.captures(&ts_query, tree.root_node(), text);

                                    let mut capture_names: Vec<String> = Vec::new();
                                    while let Some((c, _)) = captures.next() {
                                        for capture in c.captures {
                                            capture_names.push(
                                                capture.node.utf8_text(text).unwrap().to_string(),
                                            );
                                        }
                                    }

                                    Some(capture_names)
                                }
                                None => None,
                            })
                            .flatten()
                            .collect();

                        captures_by_language.insert(extension.id, captures);
                    }
                    _ => {}
                }
            }

            match query {
                Queries::ThemesSupportingCapture { capture } => println!(
                    "{}",
                    supported_captures_by_theme
                        .iter()
                        .filter(|(_, supported_captures)| supported_captures.contains(&capture))
                        .count(),
                ),
                Queries::MostCommonlyUsedCaptures => todo!(),
                Queries::LeastCommonlyUsedCaptures => todo!(),
                Queries::MostSupportedCaptures => todo!(),
                Queries::LeastSupportedCaptures => todo!(),
                Queries::LanguageCapturesSupportByTheme { theme } => todo!(),
            };
        }
    }

    // let mut total_extensions = 0;

    // println!("Total Extensions: {total_extensions}");

    // println!("Languages by Theme-Supported Captures");
    // for (lang, captures) in &captures_by_language {
    //     println!("\t{lang}: (<capture> only supported by <count> themes)");
    //     let mut captures_by_theme_support: Vec<_> = captures
    //         .iter()
    //         .filter(|capture| !capture.starts_with('_'))
    //         .map(|capture| {
    //             (
    //                 capture,
    //                 supported_captures_by_theme
    //                     .iter()
    //                     .filter(|(_, supported_captures)| supported_captures.contains(capture))
    //                     .count(),
    //             )
    //         })
    //         .collect();
    //     captures_by_theme_support.sort_by_key(|x| x.1);
    //     captures_by_theme_support.truncate(10);
    //     for (capture, count) in captures_by_theme_support {
    //         println!("\t\t{capture}: {count} ({}%)", count / theme_extensions);
    //     }
    // }

    Ok(())
}
