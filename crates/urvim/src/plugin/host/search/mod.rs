//! Plugin-facing buffer search and replacement APIs.

use std::collections::HashMap;

use bearscript::Value;
use urvim_core::buffer::{Buffer, BufferId, SearchDirection, SearchOptions, TextObjectRange};
use urvim_core::globals;

use super::super::{
    ScriptRange, buffer_id_from_value, ensure_valid_cursor, native_fn, range_from_value,
    range_to_value, unknown_buffer_error,
};

/// Builds the `urvim.search` module.
pub(in crate::plugin) fn search_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "find".to_string(),
                native_fn(
                    "search.find",
                    |buffer_id: Value, pattern: String, opts: Option<Value>| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        find_matches(buffer_id, None, &pattern, search_options(opts.as_ref())?)
                    },
                ),
            ),
            (
                "find_in_range".to_string(),
                native_fn(
                    "search.find_in_range",
                    |buffer_id: Value, range: Value, pattern: String, opts: Option<Value>| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let range = range_from_value(&range)?;
                        find_matches(
                            buffer_id,
                            Some(range),
                            &pattern,
                            search_options(opts.as_ref())?,
                        )
                    },
                ),
            ),
            (
                "replace".to_string(),
                native_fn(
                    "search.replace",
                    |buffer_id: Value,
                     pattern: String,
                     replacement: String,
                     opts: Option<Value>| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        replace_matches(
                            buffer_id,
                            &pattern,
                            &replacement,
                            search_options(opts.as_ref())?,
                        )
                    },
                ),
            ),
        ])
        .into(),
    )
}

fn search_options(opts: Option<&Value>) -> Result<SearchOptions, String> {
    let Some(opts) = opts else {
        return Ok(SearchOptions::default());
    };
    match opts {
        Value::Null => Ok(SearchOptions::default()),
        Value::Map(map) => {
            for key in map.keys() {
                if key != "case_sensitive" && key != "regex" {
                    return Err(format!("unknown search option {key}"));
                }
            }
            let case_sensitive = optional_bool(map, "case_sensitive")?.unwrap_or(true);
            let regex = optional_bool(map, "regex")?.unwrap_or(false);
            Ok(SearchOptions::new(
                SearchDirection::Forward,
                case_sensitive,
                regex,
            ))
        }
        _ => Err("search options must be a map or null".to_string()),
    }
}

fn optional_bool(map: &HashMap<String, Value>, name: &str) -> Result<Option<bool>, String> {
    match map.get(name) {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("search option {name} must be a boolean")),
    }
}

fn find_matches(
    buffer_id: BufferId,
    range: Option<ScriptRange>,
    pattern: &str,
    options: SearchOptions,
) -> Result<Value, String> {
    globals::with_buffer(buffer_id, |buffer| {
        if let Some(range) = range {
            validate_range(buffer_id, buffer, range)?;
        }
        let matches = buffer
            .find_search_matches(pattern, options)
            .map_err(|error| format!("invalid search pattern: {error}"))?;
        let values = matches
            .into_iter()
            .filter(|matched| range.is_none_or(|range| match_is_in_range(*matched, range)))
            .map(|matched| match_to_value(buffer, matched))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::List(values.into()))
    })
    .ok_or_else(|| unknown_buffer_error(buffer_id))?
}

fn replace_matches(
    buffer_id: BufferId,
    pattern: &str,
    replacement: &str,
    options: SearchOptions,
) -> Result<f64, String> {
    let replacements = globals::with_buffer(buffer_id, |buffer| {
        buffer
            .find_search_replacements(pattern, replacement, options)
            .map_err(|error| format!("invalid search pattern: {error}"))
    })
    .ok_or_else(|| unknown_buffer_error(buffer_id))??;
    let count = replacements.len();
    if count > 0 {
        let edits = replacements
            .into_iter()
            .map(|(range, text)| (range.start, range.end, text))
            .collect::<Vec<_>>();
        globals::with_buffer_mut(buffer_id, |buffer| buffer.apply_text_edits(&edits))
            .ok_or_else(|| unknown_buffer_error(buffer_id))?;
    }
    Ok(count as f64)
}

fn validate_range(buffer_id: BufferId, buffer: &Buffer, range: ScriptRange) -> Result<(), String> {
    ensure_valid_cursor(buffer_id, buffer, range.start, "range.start")?;
    ensure_valid_cursor(buffer_id, buffer, range.end, "range.end")?;
    if range.start > range.end {
        return Err("range start must be before or equal to range end".to_string());
    }
    Ok(())
}

fn match_is_in_range(matched: TextObjectRange, range: ScriptRange) -> bool {
    matched.start >= range.start && matched.end <= range.end
}

fn match_to_value(buffer: &Buffer, matched: TextObjectRange) -> Result<Value, String> {
    let text = buffer
        .text_in_range(matched.start, matched.end)
        .ok_or_else(|| "search produced an invalid match range".to_string())?;
    Ok(Value::Map(
        HashMap::from([
            (
                "range".to_string(),
                range_to_value(ScriptRange {
                    start: matched.start,
                    end: matched.end,
                }),
            ),
            (
                "text".to_string(),
                Value::String(text.into_boxed_str().into()),
            ),
        ])
        .into(),
    ))
}

#[cfg(test)]
mod tests {
    use bearscript::Engine;
    use urvim_core::buffer::Buffer;
    use urvim_core::editor_pane::EditorPane;
    use urvim_core::layout::Layout;

    use super::*;

    fn test_engine(text: &str) -> (Engine, BufferId, Layout) {
        let layout = Layout::new(EditorPane::from_buffers(vec![Buffer::from_str(text)]));
        let buffer_id = layout.active_buffer_view().buffer_id();
        let mut engine = Engine::new();
        engine.set_global("search", search_module());
        (engine, buffer_id, layout)
    }

    #[test]
    fn find_returns_literal_matches_with_text_and_utf8_byte_ranges() {
        let _guard = crate::buffer_pool_test_lock();
        let (mut engine, buffer_id, _layout) = test_engine("Été été");
        let value = engine
            .eval(&format!(
                r#"search.find({}, "été", {{ "case_sensitive": false }})"#,
                buffer_id.get()
            ))
            .unwrap();
        let Value::List(matches) = value else {
            panic!("search result should be a list");
        };

        assert_eq!(matches.len(), 2);
        let Value::Map(second) = &matches[1] else {
            panic!("match should be a map");
        };
        assert_eq!(second.get("text"), Some(&Value::String("été".into())));
        let Value::Map(range) = second.get("range").unwrap() else {
            panic!("match range should be a map");
        };
        let Value::Map(start) = range.get("start").unwrap() else {
            panic!("match start should be a map");
        };
        assert_eq!(start.get("col"), Some(&Value::Number(6.0)));
    }

    #[test]
    fn find_in_range_filters_regex_and_zero_width_matches() {
        let _guard = crate::buffer_pool_test_lock();
        let (mut engine, buffer_id, _layout) = test_engine("one 12\ntwo 34");
        let value = engine
            .eval(&format!(
                r#"[search.find_in_range({}, {{ "start": {{ "row": 1, "col": 0 }}, "end": {{ "row": 1, "col": 6 }} }}, "\\d+", {{ "regex": true }}), search.find({}, "^", {{ "regex": true }})]"#,
                buffer_id.get(),
                buffer_id.get()
            ))
            .unwrap();
        let Value::List(results) = value else {
            panic!("result should be a list");
        };
        let Value::List(ranged) = &results[0] else {
            panic!("ranged matches should be a list");
        };
        let Value::List(zero_width) = &results[1] else {
            panic!("zero-width matches should be a list");
        };

        assert_eq!(ranged.len(), 1);
        assert_eq!(zero_width.len(), 2);
    }

    #[test]
    fn replace_expands_regex_captures_and_is_one_undo_step() {
        let _guard = crate::buffer_pool_test_lock();
        let (mut engine, buffer_id, _layout) = test_engine("one two");
        let count = engine
            .eval(&format!(
                r#"search.replace({}, "(?P<word>\\w+)", "$\{{word\}}!", {{ "regex": true }})"#,
                buffer_id.get()
            ))
            .unwrap();

        assert_eq!(count, Value::Number(2.0));
        assert_eq!(
            globals::with_buffer(buffer_id, Buffer::as_str).unwrap(),
            "one! two!"
        );
        globals::with_buffer_mut(buffer_id, Buffer::undo);
        assert_eq!(
            globals::with_buffer(buffer_id, Buffer::as_str).unwrap(),
            "one two"
        );
    }

    #[test]
    fn replace_keeps_literal_replacement_syntax_literal() {
        let _guard = crate::buffer_pool_test_lock();
        let (mut engine, buffer_id, _layout) = test_engine("one");
        engine
            .eval(&format!(
                r#"search.replace({}, "one", "$1 $$")"#,
                buffer_id.get()
            ))
            .unwrap();

        assert_eq!(
            globals::with_buffer(buffer_id, Buffer::as_str).unwrap(),
            "$1 $$"
        );
    }

    #[test]
    fn rejects_invalid_patterns_ranges_options_and_buffer_ids() {
        let _guard = crate::buffer_pool_test_lock();
        let (mut engine, buffer_id, _layout) = test_engine("one");
        let scripts = [
            format!(
                r#"search.find({}, "(", {{ "regex": true }})"#,
                buffer_id.get()
            ),
            format!(
                r#"search.find({}, "one", {{ "unknown": true }})"#,
                buffer_id.get()
            ),
            format!(
                r#"search.find({}, "one", {{ "regex": "yes" }})"#,
                buffer_id.get()
            ),
            format!(
                r#"search.find_in_range({}, {{ "start": {{ "row": 0, "col": 3 }}, "end": {{ "row": 0, "col": 0 }} }}, "one")"#,
                buffer_id.get()
            ),
            "search.find(999999999, \"one\")".to_string(),
        ];

        for script in scripts {
            assert!(engine.eval(&script).is_err(), "{script} should fail");
        }
    }
}
