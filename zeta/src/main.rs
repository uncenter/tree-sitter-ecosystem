use anyhow::Result;
use clap::{arg, Parser, Subcommand, ValueEnum};
use log::debug;
use std::{
    collections::{HashMap, HashSet},
    fs,
};

use streaming_iterator::StreamingIterator;
use tree_sitter::QueryCursor;

use zeta::{
    scan,
    types::{Extension, ExtensionMetadata, ExtensionType},
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
    ByThemeSchema,
}

#[derive(Clone, ValueEnum)]
pub enum SortOrder {
    Asc,
    Desc,
    Ascending,
    Descending,
}

#[derive(Subcommand)]
pub enum Queries {
    CapturesByUsage {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    CapturesByThemeSupport {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    ThemesSupportingCapture {
        capture: String,

        #[arg(long)]
        count: bool,
    },
    LanguagesUsingCapture {
        capture: String,

        #[arg(long)]
        count: bool,
    },

    LanguagesByThemeSupport {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    ThemesByCaptureSupport {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    debug!("logger initialized");

    let args: Cli = Cli::parse();

    let cache_dir = user_dirs::cache_dir()?.join("ts-ecosystem-zeta");
    let extensions_scan_cache = cache_dir.join("extensions-scan-dump.json");
    let extensions_scan_clone = cache_dir.join("extensions-clone");

    let cache_result = || -> Result<Vec<Extension>> {
        Ok(
            fs::read_to_string(&extensions_scan_cache).and_then(|contents| {
                serde_json_lenient::from_str::<Vec<Extension>>(&contents)
                    .map_err(std::convert::Into::into)
            })?,
        )
    };

    let (extensions, cache_hit) = if args.refresh {
        (scan::extensions(&extensions_scan_clone)?, false)
    } else {
        match cache_result() {
            Ok(extensions) => (extensions, true),
            Err(_) => (scan::extensions(&extensions_scan_clone)?, false),
        }
    };

    if !cache_hit {
        fs::write(
            &extensions_scan_cache,
            serde_json_lenient::to_string(&extensions)?,
        )?;
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
            Comparisons::ByThemeSchema => {
                let mut v1_count = 0;
                let mut v2_count = 0;
                let mut invalid_count = 0;

                for extension in extensions {
                    if let ExtensionType::Theme(theme_extension) = extension.r#type {
                        for theme in theme_extension.themes {
                            match theme {
                                zeta::types::Theme::V1(_) => v1_count += 1,
                                zeta::types::Theme::V2(_) => v2_count += 1,
                                zeta::types::Theme::Invalid => invalid_count += 1,
                            }
                        }
                    }
                }

                println!(
                    "By Theme Schema:\n\tV1: {v1_count}\n\tV2: {v2_count}\n\tInvalid: {invalid_count}"
                );
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
                            .flat_map(|theme| match theme {
                                zeta::types::Theme::V1(Some(theme)) => theme
                                    .themes
                                    .iter()
                                    .flat_map(|theme| theme.style.syntax.keys())
                                    .collect::<Vec<&String>>(),
                                zeta::types::Theme::V2(Some(theme)) => theme
                                    .themes
                                    .iter()
                                    .flat_map(|theme| theme.style.syntax.keys())
                                    .collect::<Vec<&String>>(),
                                _ => Vec::new(),
                            })
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
                            .filter(|capture| !capture.starts_with('_'))
                            .collect();

                        captures_by_language.insert(extension.id, captures);
                    }
                    ExtensionType::Unknown => {}
                }
            }

            match query {
                Queries::CapturesByUsage { order, limit } => {
                    let mut capture_counts: HashMap<String, usize> = HashMap::new();
                    for capture in captures_by_language
                        .values()
                        .flat_map(|captures| {
                            // Remove duplicates *per language*, not across all languages - we want to count each capture once per language.
                            let mut seen = HashSet::new();
                            captures
                                .iter()
                                .filter(|item| seen.insert(item.clone()))
                                .collect::<Vec<_>>()
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                    {
                        *capture_counts.entry(capture).or_default() += 1;
                    }

                    let mut sorted_captures: Vec<(&String, &usize)> =
                        capture_counts.iter().collect();
                    match order {
                        SortOrder::Asc | SortOrder::Ascending => {
                            sorted_captures.sort_unstable_by(|a, b| a.1.cmp(b.1));
                        }
                        SortOrder::Desc | SortOrder::Descending => {
                            sorted_captures.sort_unstable_by(|a, b| b.1.cmp(a.1));
                        }
                    }

                    if limit != 0 {
                        sorted_captures.truncate(limit);
                    }

                    for (capture, count) in sorted_captures {
                        println!("{capture}: {count}");
                    }
                }
                Queries::CapturesByThemeSupport { order, limit } => {
                    let mut capture_counts: HashMap<String, usize> = HashMap::new();
                    for captures in supported_captures_by_theme.values() {
                        for capture in captures {
                            *capture_counts.entry(capture.clone()).or_default() += 1;
                        }
                    }

                    // display_map_sorted_truncated(capture_counts, order, limit);
                    let mut sorted_captures: Vec<(&String, &usize)> =
                        capture_counts.iter().collect();

                    match order {
                        SortOrder::Asc | SortOrder::Ascending => {
                            sorted_captures.sort_unstable_by(|a, b| a.1.cmp(b.1));
                        }
                        SortOrder::Desc | SortOrder::Descending => {
                            sorted_captures.sort_unstable_by(|a, b| b.1.cmp(a.1));
                        }
                    }

                    if limit != 0 {
                        sorted_captures.truncate(limit);
                    }

                    for (capture, count) in sorted_captures {
                        println!("{capture}: {count}");
                    }
                }

                Queries::ThemesSupportingCapture { capture, count } => {
                    let themes_with_support = supported_captures_by_theme
                        .iter()
                        .filter(|(_, supported_captures)| supported_captures.contains(&capture));
                    println!(
                        "{}",
                        if count {
                            themes_with_support.count().to_string()
                        } else {
                            themes_with_support
                                .map(|(theme, _)| theme)
                                .cloned()
                                .collect::<Vec<String>>()
                                .join("\n")
                        }
                    );
                }
                Queries::LanguagesUsingCapture { capture, count } => {
                    let languages_using_capture =
                        captures_by_language
                            .iter()
                            .filter_map(|(language, captures)| {
                                if captures.contains(&capture) {
                                    Some(language)
                                } else {
                                    None
                                }
                            });
                    println!("{}", count_or_list(languages_using_capture, count));
                }

                Queries::LanguagesByThemeSupport { order, limit } => {
                    let mut language_support_scores: HashMap<String, f64> = HashMap::new();
                    for (language, captures) in &captures_by_language {
                        let mut support_score = f64::from(0);
                        for capture in captures {
                            support_score += supported_captures_by_theme
                                .values()
                                .filter(|themes| themes.contains(capture))
                                .count() as f64;
                        }
                        support_score /= captures.len() as f64;

                        language_support_scores.insert(language.clone(), support_score);
                    }

                    // display_map_sorted_truncated(language_support_scores, order, limit);
                    let mut sorted_languages: Vec<(&String, &f64)> =
                        language_support_scores.iter().collect();

                    match order {
                        SortOrder::Asc | SortOrder::Ascending => {
                            sorted_languages.sort_unstable_by(|a, b| a.1.partial_cmp(b.1).unwrap());
                        }
                        SortOrder::Desc | SortOrder::Descending => {
                            sorted_languages.sort_unstable_by(|a, b| b.1.partial_cmp(a.1).unwrap());
                        }
                    }

                    if limit != 0 {
                        sorted_languages.truncate(limit);
                    }

                    for (language, score) in sorted_languages {
                        println!("{language}: {score}");
                    }
                }
                Queries::ThemesByCaptureSupport { order, limit } => {
                    let used_captures: HashSet<String> =
                        captures_by_language.values().flatten().cloned().collect();

                    let themes_by_used_captures_support: Vec<(&String, usize)> =
                        supported_captures_by_theme
                            .iter()
                            .map(|(theme, captures)| {
                                (
                                    theme,
                                    captures
                                        .iter()
                                        .filter(|capture| used_captures.contains(*capture))
                                        .count(),
                                )
                            })
                            .collect();

                    let mut sorted_themes: Vec<(&String, usize)> = themes_by_used_captures_support;

                    match order {
                        SortOrder::Asc | SortOrder::Ascending => {
                            sorted_themes.sort_unstable_by(|a, b| a.1.cmp(&b.1));
                        }
                        SortOrder::Desc | SortOrder::Descending => {
                            sorted_themes.sort_unstable_by(|a, b| b.1.cmp(&a.1));
                        }
                    }

                    if limit != 0 {
                        sorted_themes.truncate(limit);
                    }

                    for (theme, count) in sorted_themes {
                        println!("{theme}: {count}");
                    }
                }
            };
        }
    }

    Ok(())
}

fn count_or_list<T: ToString>(iter: impl Iterator<Item = T>, count: bool) -> String {
    if count {
        iter.count().to_string()
    } else {
        iter.map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    }
}
