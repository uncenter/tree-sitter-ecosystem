use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};

pub mod themes_v1_schema {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/themes-v1.rs"));
}
pub mod themes_v2_schema {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/themes-v2.rs"));
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionsMetadata(pub HashMap<String, ExtensionsMetadataEntry>);

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionsMetadataEntry {
    pub submodule: String,
    pub path: Option<String>,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Extension {
    pub id: String,
    pub metadata: ExtensionMetadata,
    pub builtin: bool,
    pub git_provider: Option<String>,
    pub r#type: ExtensionType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ExtensionType {
    Theme(ThemeExtension),
    Language(LanguageExtension),
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ExtensionMetadata {
    TomlManifest(TomlManifest),
    JsonManifest(JsonManifest),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TomlManifest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub schema_version: Option<usize>,
    pub authors: Vec<String>,
    pub repository: String,
    pub grammars: Option<HashMap<String, ExtensionGrammars>>,
    pub language_servers: Option<HashMap<String, ExtensionLanguageServers>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonManifest {
    pub name: String,
    pub description: Option<String>,
    pub version: String,
    pub authors: Vec<String>,
    pub repository: String,
    pub themes: Option<HashMap<String, String>>,
    pub languages: Option<HashMap<String, String>>,
    pub grammars: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionGrammars {
    pub repository: String,
    pub commit: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionLanguageServers {
    pub name: Option<String>,
    pub language: Option<String>,
    pub languages: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeExtension {
    pub themes: Vec<Theme>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Theme {
    V1(Option<themes_v1_schema::ThemeFamilyContent>),
    V2(Option<themes_v2_schema::ThemeFamilyContent>),
    Invalid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonSchema {
    #[serde(rename = "$schema")]
    pub schema: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguageExtension {
    pub languages: Vec<Language>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Language {
    pub config: LanguageConfig,
    pub highlights_queries: Option<String>,
    pub injections_queries: Option<String>,
    pub folds_queries: Option<String>,
    pub outline_queries: Option<String>,
    pub brackets_queries: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub name: String,
    pub grammar: String,
    pub path_suffixes: Option<Vec<String>>,
    pub line_comments: Option<Vec<String>>,
    pub tab_size: Option<usize>,
    pub hard_tabs: Option<bool>,
    pub first_line_pattern: Option<String>,
}

impl LanguageExtension {
    // Handle `grammars/<lang>.toml` (e.g. assembly extention).
    pub fn from_scan(languages_dir: &PathBuf) -> Result<Self> {
        let mut languages: Vec<Language> = Vec::new();

        for entry in fs::read_dir(languages_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let mut config: Option<LanguageConfig> = None;
                let mut highlights_queries = None;
                let mut injections_queries = None;
                let mut folds_queries = None;
                let mut outline_queries = None;
                let mut brackets_queries = None;

                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();
                    let file_name = entry.file_name();
                    let name = file_name.to_str().unwrap();

                    if path.is_file() {
                        match name {
                            "config.toml" => config = toml::from_str(&fs::read_to_string(path)?)?,
                            "highlights.scm" => highlights_queries = fs::read_to_string(path).ok(),
                            "injections.scm" => injections_queries = fs::read_to_string(path).ok(),
                            "folds.scm" => folds_queries = fs::read_to_string(path).ok(),
                            "outline.scm" => outline_queries = fs::read_to_string(path).ok(),
                            "brackets.scm" => brackets_queries = fs::read_to_string(path).ok(),
                            _ => {}
                        }
                    }
                }

                languages.push(Language {
                    config: config.expect("language configuration should exist"),
                    highlights_queries,
                    injections_queries,
                    folds_queries,
                    outline_queries,
                    brackets_queries,
                });
            }
        }

        Ok(Self { languages })
    }
}

impl ThemeExtension {
    pub fn from_scan(themes_dir: &PathBuf) -> Result<Self> {
        let mut themes: Vec<Theme> = Vec::new();

        for entry in fs::read_dir(themes_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|e| e == "json") {
                let contents = fs::read_to_string(&path)?;
                let json = serde_json_lenient::from_str::<JsonSchema>(&contents).ok();

                let theme_family_content = match json {
                    Some(json)
                        if json.schema.as_str() == "https://zed.dev/schema/themes/v0.1.0.json" =>
                    {
                        Theme::V1(
                            serde_json_lenient::from_str::<themes_v1_schema::ThemeFamilyContent>(
                                &contents,
                            )
                            .map_err(|e| {
                                warn!("Error parsing v1 theme: {}", e);
                            })
                            .ok(),
                        )
                    }
                    Some(json)
                        if json.schema.as_str() == "https://zed.dev/schema/themes/v0.2.0.json" =>
                    {
                        Theme::V2(
                            serde_json_lenient::from_str::<themes_v2_schema::ThemeFamilyContent>(
                                &contents,
                            )
                            .map_err(|e| {
                                warn!("Error parsing v2 theme: {}", e);
                            })
                            .ok(),
                        )
                    }
                    _ => match serde_json_lenient::from_str(&contents) {
                        Ok(v1) => Theme::V1(Some(v1)),
                        Err(_) => {
                            if let Ok(v2) = serde_json_lenient::from_str(&contents) {
                                Theme::V2(Some(v2))
                            } else {
                                warn!("Error parsing theme: {}", path.to_string_lossy());
                                Theme::Invalid
                            }
                        }
                    },
                };

                themes.push(theme_family_content);
            }
        }

        Ok(ThemeExtension { themes })
    }
}
