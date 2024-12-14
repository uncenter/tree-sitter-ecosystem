use std::{fs, path::PathBuf};

use anyhow::Result;
use git2::Repository;
use log::debug;
use url::Url;

use crate::extensions::{
    Extension, ExtensionMetadata, ExtensionType, ExtensionsMetadata, JsonManifest,
    LanguageExtension, ThemeExtension, TomlManifest,
};

pub fn clone_extensions_repository(dir: &PathBuf) -> Result<Repository> {
    let zed_extensions_repository = match Repository::open(dir) {
        Ok(repo) => repo,
        Err(_) => Repository::clone("https://github.com/zed-industries/extensions.git", dir)?,
    };
    debug!("cloned zed extensions repository to {dir:?}");

    Ok(zed_extensions_repository)
}

pub fn extensions(extensions_dir: &PathBuf) -> Result<Vec<Extension>> {
    let extensions_repository = clone_extensions_repository(extensions_dir)?;

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
                serde_json::from_str::<JsonManifest>(&fs::read_to_string(json_path)?)?,
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
            _ => ExtensionType::Unknown,
        };

        extensions.push(Extension {
            id: id.clone(),
            metadata,
            builtin: false,
            git_provider: Some(url.host_str().unwrap().to_string()),
            r#type,
        });
    }

    Ok(extensions)
}
