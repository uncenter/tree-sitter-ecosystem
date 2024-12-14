use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use git2::Repository;
use log::debug;
use serde::{Deserialize, Serialize};
use url::Url;

include!(concat!(env!("OUT_DIR"), "/schemas.rs"));

fn main() -> Result<()> {
    env_logger::init();
    debug!("logger initialized");

    let mut combined_extensions: Vec<Extension> = Vec::new();

    let zed_extensions_dir = user_dirs::cache_dir()?.join("ts-ecosystem-zeta");

    let zed_extensions_repository = Repository::open(&zed_extensions_dir).unwrap_or_else(|_| {
        Repository::clone(
            "https://github.com/zed-industries/extensions.git",
            &zed_extensions_dir,
        )
        .unwrap()
    });
    debug!("cloned zed extensions repository to {zed_extensions_dir:?}");

    let zed_extensions_metadata: ExtensionsMetadata = toml::from_str(&fs::read_to_string(
        zed_extensions_dir.join("extensions.toml"),
    )?)?;

    for (id, extension) in zed_extensions_metadata.0.iter() {
        let mut submodule = zed_extensions_repository
            .find_submodule(&extension.submodule)
            .expect("submodule for extension should exist");
        submodule.update(true, None)?;
        debug!("cloned extension submodule '{}'", &id);
        let extension_path = zed_extensions_dir
            .join(&extension.submodule)
            .join(&extension.path.clone().unwrap_or(String::new()));

        let url = Url::parse(
            &submodule
                .url()
                .expect("extension submodule should have valid url"),
        )?;

        let metadata: ExtensionMetadata = match (
            extension_path.join("extension.toml"),
            extension_path.join("extension.json"),
        ) {
            (toml_path, _) if toml_path.exists() => ExtensionMetadata::TomlManifest(
                toml::from_str::<TomlManifest>(&fs::read_to_string(toml_path)?)?,
            ),
            (_, json_path) if json_path.exists() => ExtensionMetadata::JsonManifest(
                serde_json::from_str::<JsonManifest>(&fs::read_to_string(json_path)?)?,
            ),
            _ => panic!("Extension manifest not found"),
        };

        let _type = match (
            extension_path.join("languages"),
            extension_path.join("themes"),
        ) {
            (lang_path, _) if lang_path.exists() => {
                ExtensionType::Language(LanguageExtension::from_scan(&lang_path)?)
            }
            (_, theme_path) if theme_path.exists() => {
                ExtensionType::Theme(ThemeExtension::from_scan(&theme_path)?)
            }
            _ => ExtensionType::Unknown,
        };

        combined_extensions.push(Extension {
            metadata,
            builtin: false,
            git_provider: Some(url.host_str().unwrap().to_string()),
            r#type: _type,
        })
    }

    println!("{}", serde_json::to_string_pretty(&combined_extensions)?);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionsMetadata(HashMap<String, ExtensionsMetadataEntry>);

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtensionsMetadataEntry {
    submodule: String,
    path: Option<String>,
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Extension {
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
    pub themes: Vec<Option<ThemeFamilyContent>>,
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
    name: String,
    grammar: String,
    path_suffixes: Option<Vec<String>>,
    line_comments: Option<Vec<String>>,
    tab_size: Option<usize>,
    hard_tabs: Option<bool>,
    first_line_pattern: Option<String>,
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
                })
            }
        }

        Ok(Self { languages })
    }
}

impl ThemeExtension {
    pub fn from_scan(themes_dir: &PathBuf) -> Result<Self> {
        let mut themes: Vec<Option<ThemeFamilyContent>> = Vec::new();

        for entry in fs::read_dir(themes_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|e| e == "json") {
                themes.push(serde_json::from_str(&fs::read_to_string(path)?).ok());
            }
        }

        Ok(ThemeExtension { themes })
    }
}
