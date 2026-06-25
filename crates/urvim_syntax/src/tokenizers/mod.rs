//! Builtin scanner module declarations and tokenizer dispatch.

pub mod bash;
pub mod bearscript;
pub mod bracket_folds;
pub mod c;
pub mod cmake;
pub mod cpp;
pub mod csharp;
pub mod css;
pub mod dart;
pub mod dockerfile;
pub mod elixir;
pub mod erlang;
pub mod fish;
pub mod fsharp;
pub mod go;
pub mod haskell;
pub mod html;
pub mod java;
pub mod javascript;
pub mod json;
pub mod jsx;
pub mod julia;
pub mod justfile;
pub mod kotlin;
pub mod makefile;
pub mod markdown;
pub mod nim;
pub mod ocaml;
pub mod perl;
pub mod php;
pub mod powershell;
pub mod python;
pub mod r;
pub mod ruby;
pub mod rust;
pub mod scala;
pub mod scanner;
pub mod shell;
pub mod swift;
pub mod toml;
pub mod typescript;
pub mod yaml;
pub mod zig;
pub mod zsh;

use crate::SyntaxDefinition;
use crate::SyntaxTokenizer;
use crate::state::{SyntaxLineResult, SyntaxState};

/// Dispatch to the appropriate tokenizer based on the tokenizer kind.
pub fn dispatch_builtin(
    kind: SyntaxTokenizer,
    _definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> SyntaxLineResult {
    match kind {
        SyntaxTokenizer::Bearscript => bearscript::tokenize_bearscript_line(line, state).into(),
        SyntaxTokenizer::Plaintext => (Vec::new(), SyntaxState::Plain).into(),
        SyntaxTokenizer::Bash => bash::tokenize_bash_line(line, state).into(),
        SyntaxTokenizer::C => c::tokenize_c_line(line, state),
        SyntaxTokenizer::Csharp => csharp::tokenize_csharp_line(line, state).into(),
        SyntaxTokenizer::Cmake => cmake::tokenize_cmake_line(line, state).into(),
        SyntaxTokenizer::Cpp => cpp::tokenize_cpp_line(line, state),
        SyntaxTokenizer::Css => css::tokenize_css_line(line, state),
        SyntaxTokenizer::Dart => dart::tokenize_dart_line(line, state).into(),
        SyntaxTokenizer::Dockerfile => dockerfile::tokenize_dockerfile_line(line, state).into(),
        SyntaxTokenizer::Elixir => elixir::tokenize_elixir_line(line, state).into(),
        SyntaxTokenizer::Erlang => erlang::tokenize_erlang_line(line, state).into(),
        SyntaxTokenizer::Fish => fish::tokenize_fish_line(line, state).into(),
        SyntaxTokenizer::Fsharp => fsharp::tokenize_fsharp_line(line, state).into(),
        SyntaxTokenizer::Go => go::tokenize_go_line(line, state).into(),
        SyntaxTokenizer::Haskell => haskell::tokenize_haskell_line(line, state).into(),
        SyntaxTokenizer::Html => html::tokenize_html_line(line, state),
        SyntaxTokenizer::Java => java::tokenize_java_line(line, state),
        SyntaxTokenizer::Javascript => javascript::tokenize_javascript_line(line, state),
        SyntaxTokenizer::Json => json::tokenize_json_line(line, state),
        SyntaxTokenizer::Julia => julia::tokenize_julia_line(line, state).into(),
        SyntaxTokenizer::Justfile => justfile::tokenize_justfile_line(line, state).into(),
        SyntaxTokenizer::Kotlin => kotlin::tokenize_kotlin_line(line, state).into(),
        SyntaxTokenizer::Makefile => makefile::tokenize_makefile_line(line, state).into(),
        SyntaxTokenizer::Markdown => markdown::tokenize_markdown_line(line, state),
        SyntaxTokenizer::Nim => nim::tokenize_nim_line(line, state).into(),
        SyntaxTokenizer::Ocaml => ocaml::tokenize_ocaml_line(line, state).into(),
        SyntaxTokenizer::Perl => perl::tokenize_perl_line(line, state).into(),
        SyntaxTokenizer::Php => php::tokenize_php_line(line, state).into(),
        SyntaxTokenizer::Powershell => powershell::tokenize_powershell_line(line, state).into(),
        SyntaxTokenizer::Python => python::tokenize_python_line(line, state).into(),
        SyntaxTokenizer::R => r::tokenize_r_line(line, state).into(),
        SyntaxTokenizer::Ruby => ruby::tokenize_ruby_line(line, state).into(),
        SyntaxTokenizer::Shell => shell::tokenize_shell_line(line, state).into(),
        SyntaxTokenizer::Rust => rust::tokenize_rust_line(line, state),
        SyntaxTokenizer::Scala => scala::tokenize_scala_line(line, state).into(),
        SyntaxTokenizer::Swift => swift::tokenize_swift_line(line, state).into(),
        SyntaxTokenizer::Toml => toml::tokenize_toml_line(line, state).into(),
        SyntaxTokenizer::Typescript => typescript::tokenize_typescript_line(line, state),
        SyntaxTokenizer::Yaml => yaml::tokenize_yaml_line(line, state).into(),
        SyntaxTokenizer::Zig => zig::tokenize_zig_line(line, state).into(),
        SyntaxTokenizer::Zsh => zsh::tokenize_zsh_line(line, state).into(),
    }
}
