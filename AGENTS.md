urvim is a terminal based text editor.

## Guidelines

- Write units tests whenever applicable.
- Use `cargo check` to check builds and warnings.
- Avoid `unsafe`.
- Do not use `let _ = result();` when dealing with patterns. Use `result().ok();` instead.
- Use pub instad of pub(crate).
- For complicated algorithms, make sure carefully and clearly document them with comments.
- normal application logging occurs in debug.log
- examples/demos should log to demo.log
- when asked to do a code review, also fix clippy lints
- create documentation comments for public modules, types and methods
- keep future changes separated by concern; prefer focused sub-modules over growing mixed-responsibility files
- for contained UI components, implement them as widgets (avoid ad-hoc layout-managed UI logic)
- prefer calling trait methods like `value.trait_method()` rather than `TraitType::method(value)`
- struct methods should be ordered: constructor/new, static methods, getters, setters, public methods, private methods
- when adding/update vim motions, update docs/motions.md
- when adding/update config options, update docs/config.md
- don't deprecate methods, remove them
- when describing interfaces in design.md, DO NOT include doc tests because the embedded markdown messes up design.md syntax highlighting
- when implementing methods from design.md, DO include docs tests where appropriate
- use conventional commits for commit messages
- when adding or modifying syntaxes
  - create regression tests
  - create or update appropriate fixture file for in syntax/fixtures folder
- completed specs are a historical document, not a living one. do not update them after later changes invalidate them
- format project code after edits
- instead of module_name.rs and module_name/sub_mod.rs pattern, use module_name/mod.rs and module_name/sub_mod.rs pattern
- backward compatibility isnt import because this is not a publicly release project yet
