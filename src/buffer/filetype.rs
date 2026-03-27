//! Filetype classification for buffers.
//!
//! This module provides the `Filetype` enum and helpers for resolving a
//! buffer's filetype from its filename or shebang line.

use std::path::Path;

/// Common editor-friendly filetypes supported by urvim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Filetype {
    /// Plain text fallback when no specific filetype is detected.
    PlainText,
    /// Rust source files.
    Rust,
    /// Python source files and scripts.
    Python,
    /// JavaScript source files and scripts.
    JavaScript,
    /// TypeScript source files and scripts.
    TypeScript,
    /// Lua source files and scripts.
    Lua,
    /// Shell scripts with a generic `sh` classification.
    Shell,
    /// Bash scripts.
    Bash,
    /// Zsh scripts.
    Zsh,
    /// Fish scripts.
    Fish,
    /// PowerShell scripts and manifests.
    PowerShell,
    /// Go source files.
    Go,
    /// Java source files.
    Java,
    /// C source files.
    C,
    /// C++ source files.
    Cpp,
    /// C# source files.
    CSharp,
    /// Ruby source files and scripts.
    Ruby,
    /// PHP source files and scripts.
    Php,
    /// Perl source files and scripts.
    Perl,
    /// Haskell source files.
    Haskell,
    /// Elixir source files and scripts.
    Elixir,
    /// Erlang source files and scripts.
    Erlang,
    /// OCaml source files.
    OCaml,
    /// F# source files.
    FSharp,
    /// Kotlin source files.
    Kotlin,
    /// Scala source files and scripts.
    Scala,
    /// Swift source files.
    Swift,
    /// Dart source files.
    Dart,
    /// Zig source files.
    Zig,
    /// Nim source files.
    Nim,
    /// Julia source files.
    Julia,
    /// R source files.
    R,
    /// Markdown documents.
    Markdown,
    /// JSON files.
    Json,
    /// TOML files.
    Toml,
    /// YAML files.
    Yaml,
    /// HTML files.
    Html,
    /// CSS files.
    Css,
    /// Makefiles.
    Makefile,
    /// Dockerfiles.
    Dockerfile,
    /// CMake build files.
    CMake,
    /// Justfiles.
    Justfile,
}

impl Filetype {
    /// Returns the human-readable label for this filetype.
    pub fn label(self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Lua => "Lua",
            Self::Shell => "Shell",
            Self::Bash => "Bash",
            Self::Zsh => "Zsh",
            Self::Fish => "Fish",
            Self::PowerShell => "PowerShell",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::Ruby => "Ruby",
            Self::Php => "Php",
            Self::Perl => "Perl",
            Self::Haskell => "Haskell",
            Self::Elixir => "Elixir",
            Self::Erlang => "Erlang",
            Self::OCaml => "OCaml",
            Self::FSharp => "F#",
            Self::Kotlin => "Kotlin",
            Self::Scala => "Scala",
            Self::Swift => "Swift",
            Self::Dart => "Dart",
            Self::Zig => "Zig",
            Self::Nim => "Nim",
            Self::Julia => "Julia",
            Self::R => "R",
            Self::Markdown => "Markdown",
            Self::Json => "JSON",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Makefile => "Makefile",
            Self::Dockerfile => "Dockerfile",
            Self::CMake => "CMake",
            Self::Justfile => "Justfile",
        }
    }

    /// Resolves a filetype from optional filename and first-line contents.
    ///
    /// Filename-based detection takes precedence over shebang detection.
    pub fn detect(path: Option<&Path>, first_line: Option<&str>) -> Self {
        if let Some(filetype) = path.and_then(Self::from_filename) {
            return filetype;
        }

        first_line
            .and_then(Self::from_shebang)
            .unwrap_or(Self::PlainText)
    }

    fn from_filename(path: &Path) -> Option<Self> {
        let file_name = path.file_name()?.to_string_lossy().to_lowercase();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase());

        match file_name.as_str() {
            "makefile" | "gnumakefile" => return Some(Self::Makefile),
            "dockerfile" => return Some(Self::Dockerfile),
            "justfile" => return Some(Self::Justfile),
            "cmakelists.txt" => return Some(Self::CMake),
            "gemfile" | "rakefile" | "vagrantfile" | "brewfile" => return Some(Self::Ruby),
            "procfile" => return Some(Self::Shell),
            _ => {}
        }

        match extension.as_deref() {
            Some("rs") => Some(Self::Rust),
            Some("py" | "pyw") => Some(Self::Python),
            Some("js" | "mjs" | "cjs" | "jsx") => Some(Self::JavaScript),
            Some("ts" | "tsx") => Some(Self::TypeScript),
            Some("lua") => Some(Self::Lua),
            Some("sh") => Some(Self::Shell),
            Some("bash") => Some(Self::Bash),
            Some("zsh") => Some(Self::Zsh),
            Some("fish") => Some(Self::Fish),
            Some("ps1" | "psm1" | "psd1") => Some(Self::PowerShell),
            Some("go") => Some(Self::Go),
            Some("java") => Some(Self::Java),
            Some("c" | "h") => Some(Self::C),
            Some("cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx") => Some(Self::Cpp),
            Some("cs") => Some(Self::CSharp),
            Some("rb") => Some(Self::Ruby),
            Some("php") => Some(Self::Php),
            Some("pl" | "pm") => Some(Self::Perl),
            Some("hs") => Some(Self::Haskell),
            Some("ex" | "exs") => Some(Self::Elixir),
            Some("erl") => Some(Self::Erlang),
            Some("ml" | "mli") => Some(Self::OCaml),
            Some("fs" | "fsi" | "fsx") => Some(Self::FSharp),
            Some("kt" | "kts") => Some(Self::Kotlin),
            Some("scala" | "sc") => Some(Self::Scala),
            Some("swift") => Some(Self::Swift),
            Some("dart") => Some(Self::Dart),
            Some("zig") => Some(Self::Zig),
            Some("nim") => Some(Self::Nim),
            Some("jl") => Some(Self::Julia),
            Some("r") => Some(Self::R),
            Some("md" | "markdown") => Some(Self::Markdown),
            Some("json") => Some(Self::Json),
            Some("toml") => Some(Self::Toml),
            Some("yaml" | "yml") => Some(Self::Yaml),
            Some("html" | "htm") => Some(Self::Html),
            Some("css") => Some(Self::Css),
            Some("mk") => Some(Self::Makefile),
            Some("cmake") => Some(Self::CMake),
            _ => None,
        }
    }

    fn from_shebang(first_line: &str) -> Option<Self> {
        let shebang = first_line.strip_prefix("#!")?.trim_start();
        let mut tokens = shebang.split_whitespace();
        let first = tokens.next()?;

        let interpreter = if Self::is_env_wrapper(first) {
            let mut next = tokens.next()?;
            if next == "-S" {
                next = tokens.next()?;
            }
            next
        } else {
            first
        };

        let interpreter = interpreter.rsplit('/').next().unwrap_or(interpreter);
        let interpreter = interpreter.to_lowercase();

        match interpreter.as_str() {
            value if value.starts_with("python") => Some(Self::Python),
            "node" | "nodejs" | "bun" | "deno" => Some(Self::JavaScript),
            value if value.starts_with("ruby") => Some(Self::Ruby),
            value if value.starts_with("perl") => Some(Self::Perl),
            value if value.starts_with("php") => Some(Self::Php),
            "bash" | "rbash" => Some(Self::Bash),
            "sh" => Some(Self::Shell),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            value if value.starts_with("lua") => Some(Self::Lua),
            "go" => Some(Self::Go),
            value if value.starts_with("elixir") || value.starts_with("iex") => Some(Self::Elixir),
            value if value.starts_with("erl") || value.starts_with("escript") => Some(Self::Erlang),
            value if value.starts_with("ocaml") || value.starts_with("utop") => Some(Self::OCaml),
            value if value.starts_with("fsharpi") || value.starts_with("fsi") => Some(Self::FSharp),
            value if value.starts_with("scala") => Some(Self::Scala),
            value if value.starts_with("swift") => Some(Self::Swift),
            value if value.starts_with("dart") => Some(Self::Dart),
            value if value.starts_with("zig") => Some(Self::Zig),
            value if value.starts_with("nim") => Some(Self::Nim),
            value if value.starts_with("julia") => Some(Self::Julia),
            value if value.starts_with("rscript") => Some(Self::R),
            value if value.starts_with("pwsh") || value.starts_with("powershell") => {
                Some(Self::PowerShell)
            }
            value if value.starts_with("ghci") || value.starts_with("runghc") => {
                Some(Self::Haskell)
            }
            _ => None,
        }
    }

    fn is_env_wrapper(token: &str) -> bool {
        token == "env" || token.ends_with("/env")
    }
}

#[cfg(test)]
mod tests {
    use super::Filetype;
    use std::path::Path;

    #[test]
    fn test_detect_from_filename_extension() {
        assert_eq!(
            Filetype::detect(Some(Path::new("src/main.rs")), None),
            Filetype::Rust
        );
        assert_eq!(
            Filetype::detect(Some(Path::new("main.php")), None),
            Filetype::Php
        );
        assert_eq!(
            Filetype::detect(Some(Path::new("script.ps1")), None),
            Filetype::PowerShell
        );
        assert_eq!(
            Filetype::detect(Some(Path::new("module.scala")), None),
            Filetype::Scala
        );
    }

    #[test]
    fn test_detect_from_special_filenames() {
        assert_eq!(
            Filetype::detect(Some(Path::new("Makefile")), None),
            Filetype::Makefile
        );
        assert_eq!(
            Filetype::detect(Some(Path::new("Dockerfile")), None),
            Filetype::Dockerfile
        );
        assert_eq!(
            Filetype::detect(Some(Path::new("CMakeLists.txt")), None),
            Filetype::CMake
        );
    }

    #[test]
    fn test_detect_from_shebang_env_wrapper() {
        assert_eq!(
            Filetype::detect(None, Some("#!/usr/bin/env python3 -O")),
            Filetype::Python
        );
        assert_eq!(
            Filetype::detect(
                None,
                Some("#!/usr/bin/env -S node --experimental-strip-types")
            ),
            Filetype::JavaScript
        );
        assert_eq!(Filetype::detect(None, Some("#!/bin/bash")), Filetype::Bash);
        assert_eq!(
            Filetype::detect(None, Some("#!/usr/bin/env pwsh")),
            Filetype::PowerShell
        );
    }

    #[test]
    fn test_detect_falls_back_to_plain_text() {
        assert_eq!(Filetype::detect(None, None), Filetype::PlainText);
        assert_eq!(
            Filetype::detect(Some(Path::new("README")), Some("not a shebang")),
            Filetype::PlainText
        );
    }

    #[test]
    fn test_label_is_human_readable() {
        assert_eq!(Filetype::Php.label(), "Php");
        assert_eq!(Filetype::PlainText.label(), "Plain Text");
        assert_eq!(Filetype::PowerShell.label(), "PowerShell");
    }
}
