use anyhow::Result;
use git2::Repository;
use tempfile::TempDir;

fn main() -> Result<()> {
    let mut results: Vec<Extension> = Vec::new();
    println!("test");

    let zed_extensions_dir = TempDir::new()?;
    let zed_extensions_repository = Repository::clone(
        "https://github.com/zed-industries/extensions.git",
        zed_extensions_dir,
    )?;

    let zed_extensions = zed_extensions_repository.submodules()?;

    for extension in zed_extensions {
        let path = extension.path();

        let name = match path.strip_prefix("extensions/").ok() {
            Some(name) => name,
            None => continue,
        };

        println!("{:?}", path);
    }

    Ok(())
}

pub enum Extension {
    ThemeExtension(ThemeExtension),
    Language(LanguageExtension),
    Unknown,
}

pub struct ExtensionMetadata {
    pub builtin: bool,
    pub name: String,
    pub git_provider: String,
}

pub struct ThemeExtension {
    pub metadata: ExtensionMetadata,
    pub ui_elements: Vec<String>,
    pub syntax_captures: Vec<String>,
}
pub struct LanguageExtension {
    pub metadata: ExtensionMetadata,
    pub provides_grammar: bool,
    pub provides_lsp: bool,
    pub has_highlights_queries: bool,
    pub has_injections_queries: bool,
    pub has_folds_queries: bool,
    pub has_outline_queries: bool,
    pub has_brackets_queries: bool,
}
