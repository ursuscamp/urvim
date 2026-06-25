use std::collections::{BTreeMap, HashMap};

use bearscript::Value;
use urvim_core::editor::ModeKind;
use urvim_core::globals;

use super::native_fn;

pub(in crate::plugin) fn keymaps_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "set".to_string(),
                native_fn(
                    "keymaps.set",
                    |mode: String, lhs: String, rhs: String, opts: Option<Value>| {
                        validate_keymap_opts(opts.as_ref())?;
                        let mode = mode_kind_from_keymap_string(&mode)?;
                        validate_keymap_lhs_and_rhs(&lhs, &rhs)?;
                        globals::with_plugin_keymaps_mut(|keymaps| {
                            keymap_table_mut(keymaps, mode).insert(lhs, rhs);
                        });
                        Ok(())
                    },
                ),
            ),
            (
                "delete".to_string(),
                native_fn("keymaps.delete", |mode: String, lhs: String| {
                    let mode = mode_kind_from_keymap_string(&mode)?;
                    urvim_core::editor::validate_key_string(&lhs)
                        .map_err(|error| error.to_string())?;
                    globals::with_plugin_keymaps_mut(|keymaps| {
                        keymap_table_mut(keymaps, mode).remove(&lhs);
                    });
                    Ok(())
                }),
            ),
            (
                "list".to_string(),
                native_fn("keymaps.list", |mode: Option<String>| {
                    let mode = mode
                        .as_deref()
                        .map(mode_kind_from_keymap_string)
                        .transpose()?;
                    Ok(Value::List(
                        globals::with_plugin_keymaps(|keymaps| keymap_entries(keymaps, mode))
                            .into(),
                    ))
                }),
            ),
        ])
        .into(),
    )
}

fn keymap_entries(
    keymaps: &urvim_core::globals::PluginKeymaps,
    mode: Option<ModeKind>,
) -> Vec<Value> {
    let modes: Vec<ModeKind> = mode.map(|mode| vec![mode]).unwrap_or_else(|| {
        vec![
            ModeKind::Normal,
            ModeKind::Insert,
            ModeKind::Visual,
            ModeKind::VisualLine,
            ModeKind::Resizing,
        ]
    });
    let mut entries = Vec::new();
    for mode in modes {
        for (lhs, rhs) in keymap_table(keymaps, mode) {
            entries.push(Value::Map(
                HashMap::from([
                    (
                        "mode".to_string(),
                        Value::String(keymap_mode_name(mode).into()),
                    ),
                    (
                        "lhs".to_string(),
                        Value::String(lhs.clone().into_boxed_str().into()),
                    ),
                    (
                        "rhs".to_string(),
                        Value::String(rhs.clone().into_boxed_str().into()),
                    ),
                ])
                .into(),
            ));
        }
    }
    entries
}

fn mode_kind_from_keymap_string(mode: &str) -> Result<ModeKind, String> {
    match mode {
        "normal" => Ok(ModeKind::Normal),
        "insert" => Ok(ModeKind::Insert),
        "visual" => Ok(ModeKind::Visual),
        "visual_line" | "visual-line" => Ok(ModeKind::VisualLine),
        "resizing" | "resize" => Ok(ModeKind::Resizing),
        other => Err(format!("unknown keymap mode {other}")),
    }
}

fn keymap_mode_name(mode: ModeKind) -> &'static str {
    match mode {
        ModeKind::Normal => "normal",
        ModeKind::Insert => "insert",
        ModeKind::Visual => "visual",
        ModeKind::VisualLine => "visual_line",
        ModeKind::Resizing => "resizing",
        ModeKind::Replace => "replace",
    }
}

fn validate_keymap_lhs_and_rhs(lhs: &str, rhs: &str) -> Result<(), String> {
    urvim_core::editor::validate_key_string(lhs).map_err(|error| error.to_string())?;
    let intent = urvim_core::command::parse(rhs).map_err(|error| error.to_string())?;
    super::super::validate_plugin_command_execution_intent(&intent)
}

fn validate_keymap_opts(opts: Option<&Value>) -> Result<(), String> {
    match opts {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Map(map)) if map.is_empty() => Ok(()),
        Some(Value::Map(map)) => {
            let key = map.keys().next().expect("non-empty map should have a key");
            Err(format!("unknown keymap option {key}"))
        }
        Some(_) => Err("keymap opts must be a map or null".to_string()),
    }
}

fn keymap_table(
    keymaps: &urvim_core::globals::PluginKeymaps,
    mode: ModeKind,
) -> &BTreeMap<String, String> {
    match mode {
        ModeKind::Normal => &keymaps.normal,
        ModeKind::Insert => &keymaps.insert,
        ModeKind::Visual => &keymaps.visual,
        ModeKind::VisualLine => &keymaps.visual_line,
        ModeKind::Resizing => &keymaps.resizing,
        ModeKind::Replace => &keymaps.normal,
    }
}

fn keymap_table_mut(
    keymaps: &mut urvim_core::globals::PluginKeymaps,
    mode: ModeKind,
) -> &mut BTreeMap<String, String> {
    match mode {
        ModeKind::Normal => &mut keymaps.normal,
        ModeKind::Insert => &mut keymaps.insert,
        ModeKind::Visual => &mut keymaps.visual,
        ModeKind::VisualLine => &mut keymaps.visual_line,
        ModeKind::Resizing => &mut keymaps.resizing,
        ModeKind::Replace => &mut keymaps.normal,
    }
}
