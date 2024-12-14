use std::fs;

use anyhow::Result;
use git2::Repository;
use log::debug;
use url::Url;

use crate::extensions::{
    Extension, ExtensionMetadata, ExtensionType, ExtensionsMetadata, JsonManifest,
    LanguageExtension, ThemeExtension, TomlManifest,
};

pub fn collect_external_extensions() -> Result<Vec<Extension>> {
    let mut extensions: Vec<Extension> = Vec::new();

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

    for (id, extension) in &zed_extensions_metadata.0 {
        let mut submodule = zed_extensions_repository
            .find_submodule(&extension.submodule)
            .expect("submodule for extension should exist");
        submodule.update(true, None)?;
        debug!("cloned extension submodule '{}'", &id);
        let extension_path = zed_extensions_dir
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

        extensions.push(Extension {
            id: id.clone(),
            metadata,
            builtin: false,
            git_provider: Some(url.host_str().unwrap().to_string()),
            r#type: _type,
        });
    }

    Ok(extensions)
}
