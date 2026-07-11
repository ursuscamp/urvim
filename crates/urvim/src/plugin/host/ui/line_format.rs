use std::collections::HashMap;

use bearscript::Value;
use urvim_core::ui::line_format::{
    EllipsisPlacement, FormattedLineSection, FormattedLineTemplate, LineSectionAlignment,
    LineSectionOverflow, LineSectionWidth,
};
use urvim_theme::Tag;

use super::super::native_fn;

pub(in crate::plugin::host::ui) fn line_format_module() -> Value {
    Value::Module(
        HashMap::from([(
            "render".to_string(),
            native_fn("ui.line_format.render", render_line),
        )])
        .into(),
    )
}

fn render_line(options: Value) -> Result<Value, String> {
    let Value::Map(map) = options else {
        return Err("line_format.render options must be a map".to_string());
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "width" | "values" | "sections") {
            return Err(format!("unknown line_format.render option {key}"));
        }
    }

    let width = non_negative_u16(
        map.get("width")
            .ok_or_else(|| "line_format.render requires width".to_string())?,
        "width",
    )?;
    let values = values_from_value(
        map.get("values")
            .ok_or_else(|| "line_format.render requires values".to_string())?,
    )?;
    let sections = sections_from_value(
        map.get("sections")
            .ok_or_else(|| "line_format.render requires sections".to_string())?,
    )?;

    let segments = FormattedLineTemplate::new(sections)
        .render_segments(values.iter().map(String::as_str), width)
        .map_err(|error| error.to_string())?;
    let line = segments
        .into_iter()
        .map(|segment| {
            let mut map = HashMap::from([(
                "text".to_string(),
                Value::String(segment.text.into_boxed_str().into()),
            )]);
            if let Some(style) = segment.style {
                map.insert(
                    "style".to_string(),
                    Value::String(style.as_str().to_string().into_boxed_str().into()),
                );
            }
            Value::Map(map.into())
        })
        .collect::<Vec<_>>();

    Ok(Value::List(vec![Value::List(line.into())].into()))
}

fn values_from_value(value: &Value) -> Result<Vec<String>, String> {
    let Value::List(values) = value else {
        return Err("line_format.render values must be a list of strings".to_string());
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| string_value(value, &format!("values[{index}]")))
        .collect()
}

fn sections_from_value(value: &Value) -> Result<Vec<FormattedLineSection<Option<Tag>>>, String> {
    let Value::List(sections) = value else {
        return Err("line_format.render sections must be a list".to_string());
    };
    sections
        .iter()
        .enumerate()
        .map(|(index, value)| section_from_value(value, index))
        .collect()
}

fn section_from_value(
    value: &Value,
    index: usize,
) -> Result<FormattedLineSection<Option<Tag>>, String> {
    let label = format!("sections[{index}]");
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map"));
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "style" | "width" | "alignment" | "overflow") {
            return Err(format!("unknown {label} option {key}"));
        }
    }

    let style = match map.get("style") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_tag(value, &format!("{label}.style"))?),
    };
    let width = width_from_value(
        map.get("width")
            .ok_or_else(|| format!("{label} requires width"))?,
        &format!("{label}.width"),
    )?;
    let alignment = map
        .get("alignment")
        .map(|value| {
            let value = string_value(value, &format!("{label}.alignment"))?;
            match value.as_str() {
                "left" => Ok(LineSectionAlignment::Left),
                "center" => Ok(LineSectionAlignment::Center),
                "right" => Ok(LineSectionAlignment::Right),
                other => Err(format!("unknown {label}.alignment {other}")),
            }
        })
        .transpose()?
        .unwrap_or_default();
    let overflow = map
        .get("overflow")
        .map(|value| overflow_from_value(value, &format!("{label}.overflow")))
        .transpose()?
        .unwrap_or_default();

    Ok(FormattedLineSection {
        style,
        width,
        alignment,
        overflow,
    })
}

fn width_from_value(value: &Value, label: &str) -> Result<LineSectionWidth, String> {
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map"));
    };
    for key in map.keys() {
        if key != "type" && key != "value" && key != "weight" {
            return Err(format!("unknown {label} option {key}"));
        }
    }
    let width_type = string_value(
        map.get("type")
            .ok_or_else(|| format!("{label} requires type"))?,
        &format!("{label}.type"),
    )?;
    match width_type.as_str() {
        "fixed" => {
            if map.contains_key("weight") {
                return Err(format!("{label} fixed width cannot specify weight"));
            }
            Ok(LineSectionWidth::Fixed(non_negative_u16(
                map.get("value")
                    .ok_or_else(|| format!("{label} fixed width requires value"))?,
                &format!("{label}.value"),
            )?))
        }
        "measured" => {
            if map.contains_key("value") || map.contains_key("weight") {
                return Err(format!(
                    "{label} measured width cannot specify a value or weight"
                ));
            }
            Ok(LineSectionWidth::Measured)
        }
        "flex" => {
            if map.contains_key("value") {
                return Err(format!("{label} flex width cannot specify value"));
            }
            let weight = non_negative_u16(
                map.get("weight")
                    .ok_or_else(|| format!("{label} flex width requires weight"))?,
                &format!("{label}.weight"),
            )?;
            if weight == 0 {
                return Err(format!("{label}.weight must be positive"));
            }
            Ok(LineSectionWidth::Flex(weight))
        }
        other => Err(format!("unknown {label} type {other}")),
    }
}

fn overflow_from_value(value: &Value, label: &str) -> Result<LineSectionOverflow, String> {
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map"));
    };
    for key in map.keys() {
        if key != "type" && key != "placement" {
            return Err(format!("unknown {label} option {key}"));
        }
    }
    let overflow_type = string_value(
        map.get("type")
            .ok_or_else(|| format!("{label} requires type"))?,
        &format!("{label}.type"),
    )?;
    match overflow_type.as_str() {
        "clip" => {
            if map.contains_key("placement") {
                return Err(format!("{label} clip cannot specify placement"));
            }
            Ok(LineSectionOverflow::Clip)
        }
        "ellipsis" => {
            let placement = string_value(
                map.get("placement")
                    .ok_or_else(|| format!("{label} ellipsis requires placement"))?,
                &format!("{label}.placement"),
            )?;
            let placement = match placement.as_str() {
                "start" => EllipsisPlacement::Start,
                "middle" => EllipsisPlacement::Middle,
                "end" => EllipsisPlacement::End,
                other => return Err(format!("unknown {label}.placement {other}")),
            };
            Ok(LineSectionOverflow::Ellipsis(placement))
        }
        other => Err(format!("unknown {label} type {other}")),
    }
}

fn parse_tag(value: &Value, label: &str) -> Result<Tag, String> {
    Tag::parse(string_value(value, label)?.as_str())
        .map_err(|error| format!("{label} is invalid: {error}"))
}

fn string_value(value: &Value, label: &str) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value.to_string()),
        _ => Err(format!("{label} must be a string")),
    }
}

fn non_negative_u16(value: &Value, label: &str) -> Result<u16, String> {
    let number = match value {
        Value::Number(number) => *number,
        _ => return Err(format!("{label} must be a non-negative integer")),
    };
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > u16::MAX as f64 {
        return Err(format!("{label} must be a non-negative integer"));
    }
    Ok(number as u16)
}
