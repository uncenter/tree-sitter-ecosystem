use std::{fs, path::PathBuf};

use anyhow::Result;
use git2::Repository;
use log::debug;
use url::Url;

use crate::types::{
    Extension, ExtensionMetadata, ExtensionType, ExtensionsMetadata, JsonManifest,
    LanguageExtension, ThemeExtension, TomlManifest,
};

pub fn clone_extensions_repository(dir: &PathBuf, url: &str) -> Result<Repository> {
    let repository = match Repository::open(dir) {
        Ok(repo) => repo,
        Err(_) => Repository::clone(url, dir)?,
    };
    debug!("opened {url} repository in {dir:?}");

    Ok(repository)
}

pub fn extensions(cache_dir: &PathBuf) -> Result<Vec<Extension>> {
    let extensions_dir = cache_dir.join("zed-industries/extensions");
    let extensions_repository = clone_extensions_repository(
        &extensions_dir,
        "https://github.com/zed-industries/extensions.git",
    )?;

    let extensions_metadata: ExtensionsMetadata =
        toml::from_str(&fs::read_to_string(extensions_dir.join("extensions.toml"))?)?;

    let mut extensions: Vec<Extension> = Vec::new();

    for (id, extension) in &extensions_metadata.0 {
        let mut submodule = extensions_repository
            .find_submodule(&extension.submodule)
            .expect("submodule for extension should exist");
        submodule.update(true, None)?;
        debug!("cloned extension submodule '{}'", &id);
        let extension_path = extensions_dir
            .join(&extension.submodule)
            .join(extension.path.clone().unwrap_or(String::new()));

        let builtin = extension.submodule == "extensions/zed";
        let url = Url::parse(
            submodule
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
                serde_json_lenient::from_str::<JsonManifest>(&fs::read_to_string(json_path)?)?,
            ),
            _ => panic!("Extension manifest not found"),
        };

        let r#type = match (
            extension_path.join("languages"),
            extension_path.join("themes"),
        ) {
            (lang_path, _) if lang_path.exists() => {
                ExtensionType::Language(LanguageExtension::from_scan(&lang_path)?)
            }
            (_, theme_path) if theme_path.exists() => {
                ExtensionType::Theme(ThemeExtension::from_scan(&theme_path)?)
            }
            _ => match &metadata {
                ExtensionMetadata::TomlManifest(manifest) => {
                    if manifest.grammars.is_some() || manifest.language_servers.is_some() {
                        ExtensionType::Language(LanguageExtension::default())
                    } else if manifest.slash_commands.is_some() {
                        ExtensionType::SlashCommand
                    } else if manifest.context_servers.is_some() {
                        ExtensionType::ContextServer
                    } else {
                        anyhow::bail!(
                            "Unknown extension type for extension '{}' with TOML manifest",
                            id
                        );
                    }
                }
                ExtensionMetadata::JsonManifest(manifest) => {
                    if manifest.grammars.is_some() || manifest.languages.is_some() {
                        ExtensionType::Language(LanguageExtension::default())
                    } else if manifest.themes.is_some() {
                        ExtensionType::Theme(ThemeExtension::default())
                    } else {
                        anyhow::bail!(
                            "Unknown extension type for extension '{}' with JSON manifest",
                            id
                        );
                    }
                }
            },
        };

        extensions.push(Extension {
            id: id.clone(),
            metadata,
            builtin,
            git_provider: Some(url.host_str().unwrap().to_string()),
            r#type,
        });
    }

    Ok(extensions)
}
