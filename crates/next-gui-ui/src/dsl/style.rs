#![allow(dead_code)]
//! Apply common `.ngui` style props onto Style defaults.

use next_gui_layout::CrossAlign;
use next_gui_scene::Color;

use super::ast::AstValue;
use super::DslError;
use crate::{ColumnStyle, RowStyle, StatusBarStyle, ToolbarStyle};

pub fn err_at(w_line: usize, w_col: usize, msg: impl Into<String>) -> DslError {
    DslError {
        message: msg.into(),
        line: w_line,
        column: w_col,
    }
}

pub fn as_f32(v: &AstValue, ctx: &str) -> Result<f32, String> {
    match v {
        AstValue::Number(n) => Ok(*n),
        _ => Err(format!("{ctx}: expected number")),
    }
}

pub fn as_bool(v: &AstValue, ctx: &str) -> Result<bool, String> {
    match v {
        AstValue::Bool(b) => Ok(*b),
        AstValue::Number(n) => Ok(*n != 0.0),
        _ => Err(format!("{ctx}: expected bool")),
    }
}

pub fn as_string(v: &AstValue, ctx: &str) -> Result<String, String> {
    match v {
        AstValue::String(s) => Ok(s.clone()),
        AstValue::Ident(s) => Ok(s.clone()),
        _ => Err(format!("{ctx}: expected string")),
    }
}

pub fn as_color(v: &AstValue, ctx: &str, theme: &crate::Theme) -> Result<Color, String> {
    match v {
        AstValue::Color(c) => Ok(*c),
        AstValue::Ident(name) => theme.color_by_name(name).ok_or_else(|| {
            format!("{ctx}: unknown color token `{name}` (expected ThemeColors name or #RRGGBB)")
        }),
        _ => Err(format!("{ctx}: expected #RRGGBB color or theme token name")),
    }
}

pub fn as_ident(v: &AstValue, ctx: &str) -> Result<String, String> {
    match v {
        AstValue::Ident(s) | AstValue::String(s) => Ok(s.clone()),
        _ => Err(format!("{ctx}: expected identifier")),
    }
}

pub fn as_usize(v: &AstValue, ctx: &str) -> Result<usize, String> {
    let n = as_f32(v, ctx)?;
    if n < 0.0 || n.fract() != 0.0 {
        return Err(format!("{ctx}: expected non-negative integer"));
    }
    Ok(n as usize)
}

pub fn as_u64(v: &AstValue, ctx: &str) -> Result<u64, String> {
    let n = as_f32(v, ctx)?;
    if n < 0.0 || n.fract() != 0.0 {
        return Err(format!("{ctx}: expected non-negative integer"));
    }
    Ok(n as u64)
}

pub fn as_i32(v: &AstValue, ctx: &str) -> Result<i32, String> {
    let n = as_f32(v, ctx)?;
    if n.fract() != 0.0 {
        return Err(format!("{ctx}: expected integer"));
    }
    Ok(n as i32)
}

pub fn as_u8(v: &AstValue, ctx: &str) -> Result<u8, String> {
    let n = as_f32(v, ctx)?;
    if !(0.0..=255.0).contains(&n) || n.fract() != 0.0 {
        return Err(format!("{ctx}: expected u8"));
    }
    Ok(n as u8)
}

pub fn as_string_list(v: &AstValue, ctx: &str) -> Result<Vec<String>, String> {
    match v {
        AstValue::List(items) => items
            .iter()
            .map(|i| as_string(i, ctx))
            .collect(),
        AstValue::String(s) => Ok(vec![s.clone()]),
        _ => Err(format!("{ctx}: expected string list")),
    }
}

pub fn as_number_list(v: &AstValue, ctx: &str) -> Result<Vec<f32>, String> {
    match v {
        AstValue::List(items) => items.iter().map(|i| as_f32(i, ctx)).collect(),
        _ => Err(format!("{ctx}: expected number list")),
    }
}

pub fn cross_align(v: &AstValue) -> Result<CrossAlign, String> {
    match as_ident(v, "cross_align")?.as_str() {
        "start" => Ok(CrossAlign::Start),
        "center" => Ok(CrossAlign::Center),
        "end" => Ok(CrossAlign::End),
        other => Err(format!("cross_align: expected start|center|end, got {other}")),
    }
}

pub fn apply_column_style(
    style: &mut ColumnStyle,
    props: &[(String, AstValue)],
    theme: &crate::Theme,
) -> Result<(), String> {
    for (k, v) in props {
        match k.as_str() {
            "x" => style.x = as_f32(v, k)?,
            "y" => style.y = as_f32(v, k)?,
            "width" => style.width = as_f32(v, k)?,
            "height" => style.height = as_f32(v, k)?,
            "padding" => style.padding = as_f32(v, k)?,
            "gap" => style.gap = as_f32(v, k)?,
            "background" => style.background = Some(as_color(v, k, theme)?),
            "cross_align" => style.cross_align = cross_align(v)?,
            other => return Err(format!("unknown Column property `{other}`")),
        }
    }
    Ok(())
}

pub fn apply_row_style(
    style: &mut RowStyle,
    props: &[(String, AstValue)],
    theme: &crate::Theme,
) -> Result<(), String> {
    for (k, v) in props {
        match k.as_str() {
            "x" => style.x = as_f32(v, k)?,
            "y" => style.y = as_f32(v, k)?,
            "width" => style.width = as_f32(v, k)?,
            "height" => style.height = as_f32(v, k)?,
            "padding" => style.padding = as_f32(v, k)?,
            "gap" => style.gap = as_f32(v, k)?,
            "background" => style.background = Some(as_color(v, k, theme)?),
            "cross_align" => style.cross_align = cross_align(v)?,
            other => return Err(format!("unknown Row property `{other}`")),
        }
    }
    Ok(())
}

pub fn apply_toolbar_style(
    style: &mut ToolbarStyle,
    props: &[(String, AstValue)],
    theme: &crate::Theme,
) -> Result<(), String> {
    for (k, v) in props {
        match k.as_str() {
            "x" => style.x = as_f32(v, k)?,
            "y" => style.y = as_f32(v, k)?,
            "width" => style.width = as_f32(v, k)?,
            "height" => style.height = as_f32(v, k)?,
            "padding" => style.padding = as_f32(v, k)?,
            "gap" => style.gap = as_f32(v, k)?,
            "background" => style.background = Some(as_color(v, k, theme)?),
            "cross_align" => style.cross_align = cross_align(v)?,
            other => return Err(format!("unknown Toolbar property `{other}`")),
        }
    }
    Ok(())
}

pub fn apply_status_bar_style(
    style: &mut StatusBarStyle,
    props: &[(String, AstValue)],
    theme: &crate::Theme,
) -> Result<(), String> {
    for (k, v) in props {
        match k.as_str() {
            "x" => style.x = as_f32(v, k)?,
            "y" => style.y = as_f32(v, k)?,
            "width" => style.width = as_f32(v, k)?,
            "height" => style.height = as_f32(v, k)?,
            "padding" => style.padding = as_f32(v, k)?,
            "gap" => style.gap = as_f32(v, k)?,
            "background" => style.background = Some(as_color(v, k, theme)?),
            "cross_align" => style.cross_align = cross_align(v)?,
            other => return Err(format!("unknown StatusBar property `{other}`")),
        }
    }
    Ok(())
}

/// Apply a small set of common box props onto any style that has width/height/… fields
/// via a callback pattern — used by build for Default styles.
pub fn prop_f32(
    props: &[(String, AstValue)],
    key: &str,
) -> Result<Option<f32>, String> {
    for (k, v) in props {
        if k == key {
            return Ok(Some(as_f32(v, k)?));
        }
    }
    Ok(None)
}

pub fn prop_color(
    props: &[(String, AstValue)],
    key: &str,
    theme: &crate::Theme,
) -> Result<Option<Color>, String> {
    for (k, v) in props {
        if k == key {
            return Ok(Some(as_color(v, k, theme)?));
        }
    }
    Ok(None)
}

pub fn prop_bool(
    props: &[(String, AstValue)],
    key: &str,
) -> Result<Option<bool>, String> {
    for (k, v) in props {
        if k == key {
            return Ok(Some(as_bool(v, k)?));
        }
    }
    Ok(None)
}

pub fn prop_string(
    props: &[(String, AstValue)],
    key: &str,
) -> Result<Option<String>, String> {
    for (k, v) in props {
        if k == key {
            return Ok(Some(as_string(v, k)?));
        }
    }
    Ok(None)
}

pub fn prop_usize(
    props: &[(String, AstValue)],
    key: &str,
) -> Result<Option<usize>, String> {
    for (k, v) in props {
        if k == key {
            return Ok(Some(as_usize(v, k)?));
        }
    }
    Ok(None)
}

pub fn take_prop<'a>(
    props: &'a [(String, AstValue)],
    key: &str,
) -> Option<&'a AstValue> {
    props.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Reject unknown keys given an allow-list (for widgets with limited style surface).
pub fn reject_unknown(props: &[(String, AstValue)], allowed: &[&str], kind: &str) -> Result<(), String> {
    for (k, _) in props {
        if !allowed.iter().any(|a| a == k) {
            return Err(format!("unknown {kind} property `{k}`"));
        }
    }
    Ok(())
}

/// Parse `light` / `dark` theme mode from a DSL value.
pub fn parse_theme_mode(v: &AstValue) -> Result<crate::ThemeMode, String> {
    let s = as_ident(v, "theme")?;
    match s.as_str() {
        "light" => Ok(crate::ThemeMode::Light),
        "dark" => Ok(crate::ThemeMode::Dark),
        other => Err(format!("unknown theme `{other}` (expected light|dark)")),
    }
}

/// Take and remove a `theme:` prop from a prop list (root Column only).
pub fn take_theme_prop(
    props: &mut Vec<(String, AstValue)>,
) -> Result<Option<crate::ThemeMode>, String> {
    if let Some(idx) = props.iter().position(|(k, _)| k == "theme") {
        let (_, v) = props.remove(idx);
        Ok(Some(parse_theme_mode(&v)?))
    } else {
        Ok(None)
    }
}
