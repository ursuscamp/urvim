use std::collections::{BTreeMap, HashMap};

use bearscript::Value;
use urvim_core::editor::ModeKind;
use urvim_core::globals;

use super::native_fn;

pub(in crate::plugin) fn keymaps_module(plugin: String) -> Value {
    let set_plugin = plugin.clone();
    let delete_plugin = plugin.clone();
    let list_plugin = plugin;
    Value::Module(
        HashMap::from([
            (
                "set".to_string(),
                native_fn(
                    "keymaps.set",
                    move |mode: String, lhs: String, rhs: String, opts: Option<Value>| {
                        let description = keymap_description_from_opts(opts.as_ref())?;
                        let mode = mode_kind_from_keymap_string(&mode)?;
                        validate_keymap_lhs_and_rhs(&lhs, &rhs)?;
                        globals::with_plugin_keymaps_mut(|keymaps| {
                            let mappings = keymap_table_mut(keymaps, mode);
                            if let Some(existing) = mappings.get(&lhs)
                                && existing.owner != set_plugin
                            {
                                return Err(format!(
                                    "keymap {mode:?} {lhs:?} is owned by plugin {}",
                                    existing.owner
                                ));
                            }
                            mappings.insert(
                                lhs,
                                globals::PluginKeymapEntry {
                                    owner: set_plugin.clone(),
                                    command: rhs,
                                    description,
                                },
                            );
                            Ok(())
                        })
                    },
                ),
            ),
            (
                "delete".to_string(),
                native_fn("keymaps.delete", move |mode: String, lhs: String| {
                    let mode = mode_kind_from_keymap_string(&mode)?;
                    urvim_core::editor::validate_key_string(&lhs)
                        .map_err(|error| error.to_string())?;
                    globals::with_plugin_keymaps_mut(|keymaps| {
                        let mappings = keymap_table_mut(keymaps, mode);
                        if mappings
                            .get(&lhs)
                            .is_some_and(|mapping| mapping.owner == delete_plugin)
                        {
                            mappings.remove(&lhs);
                        }
                    });
                    Ok(())
                }),
            ),
            (
                "list".to_string(),
                native_fn("keymaps.list", move |mode: Option<String>| {
                    let mode = mode
                        .as_deref()
                        .map(mode_kind_from_keymap_string)
                        .transpose()?;
                    Ok(Value::List(
                        globals::with_plugin_keymaps(|keymaps| {
                            keymap_entries(keymaps, mode, &list_plugin)
                        })
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
    plugin: &str,
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
        for (lhs, mapping) in keymap_table(keymaps, mode) {
            if mapping.owner != plugin {
                continue;
            }
            let mut entry = HashMap::from([
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
                    Value::String(mapping.command.clone().into_boxed_str().into()),
                ),
            ]);
            entry.insert(
                "desc".to_string(),
                mapping
                    .description
                    .clone()
                    .map(|value| Value::String(value.into_boxed_str().into()))
                    .unwrap_or(Value::Null),
            );
            entries.push(Value::Map(entry.into()));
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

fn keymap_description_from_opts(opts: Option<&Value>) -> Result<Option<String>, String> {
    let Some(opts) = opts else {
        return Ok(None);
    };
    match opts {
        Value::Null => Ok(None),
        Value::Map(map) => {
            for key in map.keys() {
                if key != "desc" {
                    return Err(format!("unknown keymap option {key}"));
                }
            }
            match map.get("desc") {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(description)) => Ok(Some(description.to_string())),
                Some(_) => Err("keymap option desc must be a string".to_string()),
            }
        }
        _ => Err("keymap opts must be a map or null".to_string()),
    }
}

fn keymap_table(
    keymaps: &urvim_core::globals::PluginKeymaps,
    mode: ModeKind,
) -> &BTreeMap<String, globals::PluginKeymapEntry> {
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
) -> &mut BTreeMap<String, globals::PluginKeymapEntry> {
    match mode {
        ModeKind::Normal => &mut keymaps.normal,
        ModeKind::Insert => &mut keymaps.insert,
        ModeKind::Visual => &mut keymaps.visual,
        ModeKind::VisualLine => &mut keymaps.visual_line,
        ModeKind::Resizing => &mut keymaps.resizing,
        ModeKind::Replace => &mut keymaps.normal,
    }
}
