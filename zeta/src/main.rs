#![warn(clippy::pedantic, clippy::all)]

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
    types::{Extension, ExtensionMetadata, ExtensionType, Theme},
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
    Show {
        id: String,
    },
    /// Find extensions matching certain criteria.
    Find {
        #[arg(long)]
        manifest: Option<BasicManifestType>,

        #[arg(long)]
        r#type: Option<BasicExtensionType>,

        #[arg(long)]
        git_provider: Option<String>,

        #[arg(long)]
        theme_schema: Option<BasicThemeSchema>,

        #[arg(long)]
        builtin: Option<bool>,

        #[arg(long)]
        count: bool,
    },
    /// Analyze extensions with various queries, mostly related to captures.
    Analyze {
        #[command(subcommand)]
        query: AnalysisQuery,
    },
}

#[derive(Clone, ValueEnum)]
pub enum SortOrder {
    Asc,
    Desc,
    Ascending,
    Descending,
}

#[derive(Clone, ValueEnum)]
pub enum BasicManifestType {
    Json,
    Toml,
}

#[derive(Clone, ValueEnum)]
pub enum BasicExtensionType {
    Theme,
    Language,
    SlashCommand,
    ContextServer,
}

#[derive(Clone, ValueEnum)]
pub enum BasicThemeSchema {
    V1,
    V2,
    Other,
}

#[derive(Subcommand)]
pub enum AnalysisQuery {
    /// Query the most (order: desc) or least (order: asc) used captures in language extensions.
    CapturesByUsage {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Query the most (order: desc) or least (order: asc) supported captures in theme extensions.
    CapturesByThemeSupport {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Query the themes supporting a specific capture.
    ThemesSupportingCapture {
        capture: String,

        #[arg(long)]
        count: bool,
    },
    /// Query the languages using a specific capture.
    LanguagesUsingCapture {
        capture: String,

        #[arg(long)]
        count: bool,
    },

    /// Roughly score and rank languages by the depth (average number of themes supporting each capture used in a language) and breadth (number of themes supporting at least one capture) of theme support.
    /// The score is calculated as 7 * depth / number of captures + 3 * breadth.
    /// The best languages will have a high score (order: desc) and the worst languages will have a low score (order: asc).
    LanguagesByThemeSupport {
        order: SortOrder,

        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Query the themes supporting the most (order: desc) or least (order: asc) *USED* captures. Captures are considered used if they are used in any language extension.
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

    let cache_result = || -> Result<Vec<Extension>> {
        Ok(
            fs::read_to_string(&extensions_scan_cache).and_then(|contents| {
                serde_json_lenient::from_str::<Vec<Extension>>(&contents)
                    .map_err(std::convert::Into::into)
            })?,
        )
    };

    let (extensions, cache_hit) = if args.refresh {
        (scan::extensions(&cache_dir)?, false)
    } else {
        match cache_result() {
            Ok(extensions) => (extensions, true),
            Err(_) => (scan::extensions(&cache_dir)?, false),
        }
    };

    if !cache_hit {
        fs::write(
            &extensions_scan_cache,
            serde_json_lenient::to_string(&extensions)?,
        )?;
    }

    match args.command {
        Commands::Analyze { query } => handle_query(query, extensions),
        Commands::Find {
            manifest,
            r#type,
            git_provider,
            theme_schema,
            builtin,
            count,
        } => {
            let matching: Vec<Extension> = extensions
                .into_iter()
                .filter(|extension| {
                    match &manifest {
                        Some(BasicManifestType::Json) => {
                            if let ExtensionMetadata::TomlManifest(_) = extension.metadata {
                                return false;
                            };
                        }
                        Some(BasicManifestType::Toml) => {
                            if let ExtensionMetadata::JsonManifest(_) = extension.metadata {
                                return false;
                            };
                        }
                        None => {}
                    };

                    // Check if the extension has the same type as the provided type.
                    match &r#type {
                        Some(extension_type) => match extension_type {
                            BasicExtensionType::Theme => {
                                if !matches!(extension.r#type, ExtensionType::Theme(_)) {
                                    return false;
                                }
                            }
                            BasicExtensionType::Language => {
                                if !matches!(extension.r#type, ExtensionType::Language(_)) {
                                    return false;
                                }
                            }
                            BasicExtensionType::SlashCommand => {
                                if !matches!(extension.r#type, ExtensionType::SlashCommand) {
                                    return false;
                                }
                            }
                            BasicExtensionType::ContextServer => {
                                if !matches!(extension.r#type, ExtensionType::ContextServer) {
                                    return false;
                                }
                            }
                        },
                        None => {}
                    };

                    // Check if the extension has the same git provider as the provided git provider.
                    if let Some(provider) = &git_provider {
                        if extension.git_provider.as_deref() != Some(provider) {
                            return false;
                        }
                    }

                    // Check if the extension has the same theme schema as the provided theme schema.
                    // This is a bit more complex because the theme schema is nested.
                    if let Some(schema) = &theme_schema {
                        if let ExtensionType::Theme(theme_extension) = &extension.r#type {
                            if theme_extension.themes.iter().any(|theme| match theme {
                                Some(Theme::V1(Some(_))) => matches!(schema, BasicThemeSchema::V1),
                                Some(Theme::V2(Some(_))) => matches!(schema, BasicThemeSchema::V2),
                                _ => false,
                            }) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }

                    if let Some(builtin) = builtin {
                        if extension.builtin != builtin {
                            return false;
                        }
                    }

                    true
                })
                .collect();

            println!("{}", count_or_list(matching, count));
        }
        Commands::Show { id } => {
            let extension = extensions
                .into_iter()
                .find(|extension| extension.id == id)
                .expect("extension not found");

            println!("{}", serde_json_lenient::to_string_pretty(&extension)?);
        }
    }

    Ok(())
}

fn count_or_list<T: ToString>(items: Vec<T>, count: bool) -> String {
    if count {
        items.len().to_string()
    } else {
        items
            .into_iter()
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    }
}

fn sort_truncate_display_hashmap(map: &HashMap<String, usize>, order: &SortOrder, limit: usize) {
    let mut sorted_map: Vec<(&String, &usize)> = map.iter().collect();

    match order {
        SortOrder::Asc | SortOrder::Ascending => {
            sorted_map.sort_unstable_by(|a, b| a.1.cmp(b.1));
        }
        SortOrder::Desc | SortOrder::Descending => {
            sorted_map.sort_unstable_by(|a, b| b.1.cmp(a.1));
        }
    }

    if limit != 0 {
        sorted_map.truncate(limit);
    }

    for (key, value) in sorted_map {
        println!("{key}: {value}");
    }
}

fn extract_capture_names(
    source_code: &str,
    ts_parser: &mut tree_sitter::Parser,
    ts_query: &tree_sitter::Query,
) -> Option<Vec<String>> {
    let tree = ts_parser.parse(source_code, None)?;
    let mut cursor = QueryCursor::new();
    let text_bytes = source_code.as_bytes();
    let mut captures = cursor.captures(ts_query, tree.root_node(), text_bytes);

    let mut capture_names: Vec<String> = Vec::new();
    while let Some((c, _)) = captures.next() {
        for capture in c.captures {
            capture_names.push(capture.node.utf8_text(text_bytes).unwrap().to_string());
        }
    }

    Some(capture_names)
}

fn handle_query(query: AnalysisQuery, extensions: Vec<Extension>) {
    let mut supported_captures_by_theme: HashMap<String, Vec<String>> = HashMap::new();
    let mut captures_by_language: HashMap<String, Vec<String>> = HashMap::new();

    let mut ts_parser = tree_sitter::Parser::new();
    ts_parser
        .set_language(&tree_sitter_query::LANGUAGE.into())
        .expect("should load tree-sitter-query grammar");

    let ts_query = tree_sitter::Query::new(
        &tree_sitter_query::LANGUAGE.into(),
        "(capture (identifier) @name)",
    )
    .expect("tree-sitter-query capture query should build");

    for extension in extensions {
        match extension.r#type {
            ExtensionType::Theme(theme_extension) => {
                let mut syntax_captures: Vec<String> = theme_extension
                    .themes
                    .iter()
                    .flat_map(|theme| match theme {
                        Some(Theme::V1(Some(theme))) => theme
                            .themes
                            .iter()
                            .flat_map(|theme| theme.style.syntax.keys())
                            .collect::<Vec<&String>>(),
                        Some(Theme::V2(Some(theme))) => theme
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
                    .filter_map(|language| {
                        if let Some(highlights) = &language.highlights_queries {
                            extract_capture_names(highlights, &mut ts_parser, &ts_query)
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .filter(|capture| !capture.starts_with('_'))
                    .collect();

                captures_by_language.insert(extension.id, captures);
            }
            ExtensionType::SlashCommand | ExtensionType::ContextServer => {}
        }
    }

    match query {
        AnalysisQuery::CapturesByUsage { order, limit } => {
            let mut capture_counts: HashMap<String, usize> = HashMap::new();
            let captures_by_language: Vec<String> = captures_by_language
                .into_values()
                .flat_map(|mut values| {
                    let mut seen = HashSet::new();
                    values
                        .drain(..)
                        .filter(|item| seen.insert(item.clone()))
                        .collect::<Vec<String>>()
                })
                .collect();

            for capture in captures_by_language {
                *capture_counts.entry(capture).or_default() += 1;
            }

            sort_truncate_display_hashmap(&capture_counts, &order, limit);
        }
        AnalysisQuery::CapturesByThemeSupport { order, limit } => {
            let mut capture_counts: HashMap<String, usize> = HashMap::new();
            for captures in supported_captures_by_theme.values() {
                for capture in captures {
                    *capture_counts.entry(capture.clone()).or_default() += 1;
                }
            }

            sort_truncate_display_hashmap(&capture_counts, &order, limit);
        }

        AnalysisQuery::ThemesSupportingCapture { capture, count } => {
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
        AnalysisQuery::LanguagesUsingCapture { capture, count } => {
            let languages_using_capture = captures_by_language
                .iter()
                .filter_map(|(language, captures)| {
                    if captures.contains(&capture) {
                        Some(language)
                    } else {
                        None
                    }
                })
                .collect();

            println!("{}", count_or_list(languages_using_capture, count));
        }

        AnalysisQuery::LanguagesByThemeSupport { order, limit } => {
            let mut language_support_scores: HashMap<String, usize> = HashMap::new();

            for (language, captures) in &captures_by_language {
                // Average number of themes supporting each capture.
                let capture_support_depth: usize = captures
                    .iter()
                    .map(|capture| {
                        supported_captures_by_theme
                            .values()
                            .filter(|captures| captures.contains(capture))
                            .count()
                    })
                    .sum();

                // Count of themes supporting at least one capture.
                let theme_support_breadth = supported_captures_by_theme
                    .values()
                    .filter(|supported_captures| {
                        supported_captures
                            .iter()
                            .any(|capture| captures.contains(capture))
                    })
                    .count();

                let scaled_capture_support_depth = 7 * capture_support_depth / captures.len();
                let scaled_theme_support_breadth = 3 * theme_support_breadth;

                let support_score = scaled_capture_support_depth + scaled_theme_support_breadth;

                language_support_scores.insert(language.clone(), support_score);
            }

            sort_truncate_display_hashmap(&language_support_scores, &order, limit);
        }

        AnalysisQuery::ThemesByCaptureSupport { order, limit } => {
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
