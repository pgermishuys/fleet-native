//! Which language colours a file (`lib/code-editor/languages.ts`, `languageFor`): Rune's own file-name table
//! (`lang_for_path`), with Fleet's aliases for the names CodeMirror's language data doesn't know by extension (`.csproj` is XML,
//! `.zsh` shell, `.env` properties). A dotfile such as `.env.local` counts by its parts. `None` draws plain.
use rune_kit::syntax::lang_for_path;

/// Extension → the language that draws it (`EXTENSION_ALIASES`).
const ALIASES: [(&str, &str); 16] = [
    ("csproj", "xml"),
    ("fsproj", "xml"),
    ("vbproj", "xml"),
    ("props", "xml"),
    ("targets", "xml"),
    ("slnx", "xml"),
    ("nuspec", "xml"),
    ("xaml", "xml"),
    ("axaml", "xml"),
    ("resx", "xml"),
    ("plist", "xml"),
    ("jsonc", "json"),
    ("json5", "json"),
    ("zsh", "sh"),
    ("env", "properties"),
    ("editorconfig", "properties"),
];

/// The language name Rune's highlighter takes for `path`, or `None` for plain text.
pub fn language_for(path: &str) -> Option<&'static str> {
    if let Some(found) = lang_for_path(path) {
        return Some(found);
    }
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    // The last part that has an alias wins, as the parts are tried from the end.
    let alias = name.split('.').filter(|p| !p.is_empty()).rev().find_map(|part| ALIASES.iter().find(|(k, _)| *k == part))?;
    lang_for_path(&format!("file.{}", alias.1)).or(Some(alias.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    // completion.test.ts, "languageFor": the files and the language CodeMirror names; Rune names them its own way.
    #[test]
    fn language_for_names_a_file_s_language() {
        for (path, lang) in [
            ("src/app.ts", "typescript"),
            ("src/App.vue", "vue"),
            ("src/Program.cs", "csharp"),
            ("WeaveFleet.Api.csproj", "xml"),
            ("Directory.Build.props", "xml"),
            ("main.go", "go"),
            ("script.py", "python"),
            ("lib.rs", "rust"),
            ("README.md", "markdown"),
            ("config.yml", "yml"),
            ("Dockerfile", "dockerfile"),
            (".env.local", "properties"),
        ] {
            assert_eq!(language_for(path), Some(lang), "{path}");
            // Rune has no lexer for properties files: the name is Fleet's, and the file draws plain (docs/parity/11-files.md, row 5.3).
            assert!(lang == "properties" || rune_kit::syntax::known(lang), "Rune's highlighter colours {lang}");
        }
    }

    #[test]
    fn language_for_is_none_for_plain_text() {
        assert_eq!(language_for("notes.txt"), None);
        assert_eq!(language_for("LICENSE"), None);
    }
}
