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
- prefer calling trait methods like `value.trait_method()` rather than `TraitType::method(value)`
- struct methods should be ordered: constructor/new, static methods, getters, setters, public methods, private methods
