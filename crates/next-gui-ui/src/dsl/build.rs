//! AST → retained [`Ui`] via FlexBuilder.

use std::collections::HashMap;

use next_gui_scene::Color;

use super::ast::{AstValue, AstWidget, Document};
use super::style::{
    apply_column_style, apply_row_style, apply_status_bar_style, apply_toolbar_style, as_bool,
    as_color, as_f32, as_ident, as_number_list, as_string, as_string_list, as_u64, as_usize,
    err_at, prop_bool, prop_color, prop_f32, prop_string, prop_usize, take_prop, take_theme_prop,
};
use super::{DslError, LoadedUi};
use crate::{
    AlarmEntry, AlarmListStyle, AlarmSeverity, AlertStyle, AlertTone, AutocompleteStyle,
    BadgeKind, BadgeStyle, BreadcrumbStyle, ButtonGroupStyle, CalendarStyle, CardStyle,
    ChartStyle, ChartTile, CheckboxStyle, CollapsePanel, CollapseStyle, ColorPickerStyle,
    ContextMenuStyle, DateTimeStyle, DateTimeValue, DescriptionItem, DescriptionsStyle,
    DrawerStyle, EmptyStyle, FlexBuilder, FormRule, FormStyle, GaugeStyle, GridStyle, IconName,
    ImageStyle, InputNumberStyle, ListStyle, MenuItem, MenuStyle, ModalStyle,
    NotificationTone, PaginationStyle, PanelStyle, PopconfirmStyle, PopoverStyle, ProgressStyle,
    PropertyGridStyle, PropertyRow, PropertyValue, RadioStyle, ResultStatus, ResultStyle,
    ScrollAreaStyle, SegmentedStyle, SelectStyle, SliderStyle, SpaceDirection, SpaceStyle,
    SpinStyle, SplitterStyle, StatisticStyle, StepItem, StepsStyle, SwitchStyle, TableColumn,
    TableStyle, TabsStyle, TagStyle, TextAreaStyle, TextInputStyle, Theme, ThemeMode,
    TimelineItem, TimelineStyle, TransferItem, TransferStyle, TreeNode, TreeSelectStyle, TreeStyle,
    TreeTableNode, TreeTableStyle, TrendStyle, Ui, UploadStyle, WidgetId,
};

pub fn document_to_ui(doc: Document) -> Result<LoadedUi, DslError> {
    let mut ui = Ui::new();
    let mut names = HashMap::new();

    // Theme: document-level `theme: …` first; root Column `theme:` prop overrides.
    let mut root_props = doc.root.props.clone();
    let mut theme_mode = doc.theme;
    let root_theme = take_theme_prop(&mut root_props)
        .map_err(|m| err_at(doc.root.line, doc.root.column, m))?;
    if let Some(mode) = root_theme {
        theme_mode = Some(mode);
    }
    if let Some(mode) = theme_mode {
        ui.set_theme(match mode {
            ThemeMode::Light => Theme::light(),
            ThemeMode::Dark => Theme::dark(),
        });
    }

    let style = column_style_from(ui.theme(), &doc.root, &root_props)?;
    let root_name = doc.root.name.clone();
    let children = doc.root.children.clone();
    let mut build_err: Option<DslError> = None;
    let root_id = ui.column(style, |b| {
        if let Err(e) = build_children(b, &mut names, &children) {
            build_err = Some(e);
        }
    });
    if let Some(e) = build_err {
        return Err(e);
    }
    if let Some(n) = root_name {
        names.insert(n, root_id);
    }
    for overlay in doc.overlays {
        apply_overlay(&mut ui, &mut names, &overlay)?;
    }
    Ok(LoadedUi { ui, names })
}

fn column_style_from(
    theme: &Theme,
    w: &AstWidget,
    props: &[(String, AstValue)],
) -> Result<crate::ColumnStyle, DslError> {
    let mut style = crate::ColumnStyle::from_theme(theme);
    apply_column_style(&mut style, props, theme).map_err(|m| err_at(w.line, w.column, m))?;
    Ok(style)
}

fn map_err(w: &AstWidget, r: Result<(), String>) -> Result<(), DslError> {
    r.map_err(|m| err_at(w.line, w.column, m))
}

fn register(names: &mut HashMap<String, WidgetId>, name: &Option<String>, id: WidgetId) {
    if let Some(n) = name {
        names.insert(n.clone(), id);
    }
}

fn build_children(
    builder: &mut FlexBuilder<'_>,
    names: &mut HashMap<String, WidgetId>,
    children: &[AstWidget],
) -> Result<(), DslError> {
    for child in children {
        build_widget(builder, names, child)?;
    }
    Ok(())
}

fn apply_overlay(
    ui: &mut Ui,
    names: &mut HashMap<String, WidgetId>,
    w: &AstWidget,
) -> Result<(), DslError> {
    match w.kind.as_str() {
        "Modal" => {
            let title = arg_string(w, 0, "title")?;
            let body = arg_string(w, 1, "body").unwrap_or_default();
            let primary = arg_string(w, 2, "primary").unwrap_or_else(|_| "OK".into());
            let secondary = prop_string(&w.props, "secondary")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let open = prop_bool(&w.props, "open")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(false);
            let id = ui.modal(title, body, primary, secondary, ModalStyle::from_theme(ui.theme()));
            if open {
                ui.set_modal_open(id, true);
            }
            register(names, &w.name, id);
            Ok(())
        }
        "Drawer" => {
            let title = arg_string(w, 0, "title")?;
            let body = arg_string(w, 1, "body").unwrap_or_default();
            let open = prop_bool(&w.props, "open")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(false);
            let id = ui.drawer(title, body, DrawerStyle::from_theme(ui.theme()));
            if open {
                ui.set_drawer_open(id, true);
            }
            register(names, &w.name, id);
            Ok(())
        }
        "Notify" => {
            let msg = arg_string(w, 0, "message")?;
            let tone = match prop_string(&w.props, "tone")
                .map_err(|m| err_at(w.line, w.column, m))?
                .as_deref()
            {
                None | Some("info") => NotificationTone::Info,
                Some("success") => NotificationTone::Success,
                Some("warning") => NotificationTone::Warning,
                Some("error") => NotificationTone::Error,
                Some(other) => {
                    return Err(err_at(
                        w.line,
                        w.column,
                        format!("unknown Notify tone `{other}`"),
                    ));
                }
            };
            let _id = ui.notify(msg, tone);
            Ok(())
        }
        other => Err(err_at(
            w.line,
            w.column,
            format!("unknown overlay `{other}`"),
        )),
    }
}

fn arg_string(w: &AstWidget, idx: usize, label: &str) -> Result<String, DslError> {
    w.args
        .get(idx)
        .ok_or_else(|| {
            err_at(
                w.line,
                w.column,
                format!("{}: missing arg {idx} ({label})", w.kind),
            )
        })
        .and_then(|v| as_string(v, label).map_err(|m| err_at(w.line, w.column, m)))
}

fn first_string_arg_or(w: &AstWidget, default: &str) -> Result<String, DslError> {
    if let Some(v) = w.args.first() {
        as_string(v, "arg0").map_err(|m| err_at(w.line, w.column, m))
    } else {
        Ok(default.to_string())
    }
}

fn parse_tree_nodes(v: &AstValue) -> Result<Vec<TreeNode>, String> {
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("tree: expected list of nodes".into()),
    };
    list.iter().map(parse_tree_node).collect()
}

fn parse_tree_node(v: &AstValue) -> Result<TreeNode, String> {
    match v {
        AstValue::Object(fields) => {
            let id = fields
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| as_u64(v, "id"))
                .transpose()?
                .unwrap_or(0);
            let label = fields
                .iter()
                .find(|(k, _)| k == "label")
                .map(|(_, v)| as_string(v, "label"))
                .transpose()?
                .unwrap_or_default();
            let children = fields
                .iter()
                .find(|(k, _)| k == "children")
                .map(|(_, v)| parse_tree_nodes(v))
                .transpose()?
                .unwrap_or_default();
            let expanded = fields
                .iter()
                .find(|(k, _)| k == "expanded")
                .map(|(_, v)| as_bool(v, "expanded"))
                .transpose()?
                .unwrap_or(!children.is_empty());
            Ok(TreeNode {
                id,
                label,
                children,
                expanded,
            })
        }
        AstValue::List(items) if items.len() >= 2 => {
            let id = as_u64(&items[0], "id")?;
            let label = as_string(&items[1], "label")?;
            let children = if let Some(c) = items.get(2) {
                parse_tree_nodes(c)?
            } else {
                Vec::new()
            };
            Ok(TreeNode::new(id, label, children))
        }
        _ => Err("tree node: expected {id,label,children} or [id,label,children]".into()),
    }
}

fn parse_menu_items(v: &AstValue) -> Result<Vec<MenuItem>, String> {
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("menu items: expected list".into()),
    };
    let mut out = Vec::new();
    for (i, item) in list.iter().enumerate() {
        match item {
            AstValue::String(s) => out.push(MenuItem::new(i as u64, s.clone())),
            AstValue::Object(fields) => {
                let id = fields
                    .iter()
                    .find(|(k, _)| k == "id")
                    .map(|(_, v)| as_u64(v, "id"))
                    .transpose()?
                    .unwrap_or(i as u64);
                let label = fields
                    .iter()
                    .find(|(k, _)| k == "label")
                    .map(|(_, v)| as_string(v, "label"))
                    .transpose()?
                    .unwrap_or_default();
                let enabled = fields
                    .iter()
                    .find(|(k, _)| k == "enabled")
                    .map(|(_, v)| as_bool(v, "enabled"))
                    .transpose()?
                    .unwrap_or(true);
                out.push(if enabled {
                    MenuItem::new(id, label)
                } else {
                    MenuItem::disabled(id, label)
                });
            }
            _ => return Err("menu item: expected string or {id,label}".into()),
        }
    }
    Ok(out)
}

fn parse_table_columns(v: &AstValue) -> Result<Vec<TableColumn>, String> {
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("columns: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        match item {
            AstValue::String(s) => out.push(TableColumn::new(s.clone(), 100.0)),
            AstValue::Object(fields) => {
                let title = fields
                    .iter()
                    .find(|(k, _)| k == "title" || k == "label")
                    .map(|(_, v)| as_string(v, "title"))
                    .transpose()?
                    .unwrap_or_default();
                let width = fields
                    .iter()
                    .find(|(k, _)| k == "width")
                    .map(|(_, v)| as_f32(v, "width"))
                    .transpose()?
                    .unwrap_or(100.0);
                out.push(TableColumn::new(title, width));
            }
            AstValue::List(pair) if !pair.is_empty() => {
                let title = as_string(&pair[0], "title")?;
                let width = pair
                    .get(1)
                    .map(|v| as_f32(v, "width"))
                    .transpose()?
                    .unwrap_or(100.0);
                out.push(TableColumn::new(title, width));
            }
            _ => return Err("column: expected string, {title,width}, or [title,width]".into()),
        }
    }
    Ok(out)
}

fn parse_table_rows(v: &AstValue) -> Result<Vec<Vec<String>>, String> {
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("rows: expected list of lists".into()),
    };
    list.iter()
        .map(|row| match row {
            AstValue::List(cells) => cells
                .iter()
                .map(|c| as_string(c, "cell"))
                .collect::<Result<Vec<_>, _>>(),
            AstValue::String(s) => Ok(vec![s.clone()]),
            _ => Err("row: expected list of strings".into()),
        })
        .collect()
}

fn parse_alert_tone(s: &str) -> Result<AlertTone, String> {
    match s {
        "info" => Ok(AlertTone::Info),
        "success" => Ok(AlertTone::Success),
        "warning" => Ok(AlertTone::Warning),
        "error" => Ok(AlertTone::Error),
        other => Err(format!("unknown Alert tone `{other}`")),
    }
}

fn parse_alarm_severity(s: &str) -> Result<AlarmSeverity, String> {
    match s.to_ascii_lowercase().as_str() {
        "critical" | "crit" => Ok(AlarmSeverity::Critical),
        "major" | "maj" => Ok(AlarmSeverity::Major),
        "minor" | "min" => Ok(AlarmSeverity::Minor),
        "warning" | "warn" => Ok(AlarmSeverity::Warning),
        "info" => Ok(AlarmSeverity::Info),
        other => Err(format!("unknown alarm severity `{other}`")),
    }
}

fn parse_alarm_entries(v: &AstValue) -> Result<Vec<AlarmEntry>, String> {
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("alarms: expected list".into()),
    };
    let mut out = Vec::new();
    for (i, item) in list.iter().enumerate() {
        let AstValue::Object(fields) = item else {
            return Err("alarm: expected object".into());
        };
        let id = fields
            .iter()
            .find(|(k, _)| k == "id")
            .map(|(_, v)| as_u64(v, "id"))
            .transpose()?
            .unwrap_or(i as u64);
        let severity = fields
            .iter()
            .find(|(k, _)| k == "severity")
            .map(|(_, v)| as_ident(v, "severity"))
            .transpose()?
            .map(|s| parse_alarm_severity(&s))
            .transpose()?
            .unwrap_or(AlarmSeverity::Info);
        let message = fields
            .iter()
            .find(|(k, _)| k == "message")
            .map(|(_, v)| as_string(v, "message"))
            .transpose()?
            .unwrap_or_default();
        let source = fields
            .iter()
            .find(|(k, _)| k == "source")
            .map(|(_, v)| as_string(v, "source"))
            .transpose()?
            .unwrap_or_default();
        let time = fields
            .iter()
            .find(|(k, _)| k == "time")
            .map(|(_, v)| as_string(v, "time"))
            .transpose()?
            .unwrap_or_default();
        let mut e = AlarmEntry::new(id, severity, message, source, time);
        if let Some((_, v)) = fields.iter().find(|(k, _)| k == "acked") {
            e.acked = as_bool(v, "acked")?;
        }
        out.push(e);
    }
    Ok(out)
}

fn parse_form_rules(v: &AstValue) -> Result<Vec<FormRule>, String> {
    let list = match v {
        AstValue::List(items) => items,
        AstValue::String(s) | AstValue::Ident(s) => {
            return Ok(vec![rule_from_ident(s)?]);
        }
        _ => return Err("rules: expected list".into()),
    };
    list.iter()
        .map(|item| match item {
            AstValue::String(s) | AstValue::Ident(s) => rule_from_ident(s),
            AstValue::Object(fields) => {
                let kind = fields
                    .iter()
                    .find(|(k, _)| k == "kind" || k == "type")
                    .map(|(_, v)| as_ident(v, "kind"))
                    .transpose()?
                    .unwrap_or_else(|| "required".into());
                let message = fields
                    .iter()
                    .find(|(k, _)| k == "message")
                    .map(|(_, v)| as_string(v, "message"))
                    .transpose()?
                    .unwrap_or_else(|| "invalid".into());
                match kind.as_str() {
                    "required" => Ok(FormRule::required(message)),
                    "digits" => Ok(FormRule::digits(message)),
                    "min_length" | "minLength" => {
                        let min = fields
                            .iter()
                            .find(|(k, _)| k == "min")
                            .map(|(_, v)| as_usize(v, "min"))
                            .transpose()?
                            .unwrap_or(1);
                        Ok(FormRule::min_length(min, message))
                    }
                    "max_length" | "maxLength" => {
                        let max = fields
                            .iter()
                            .find(|(k, _)| k == "max")
                            .map(|(_, v)| as_usize(v, "max"))
                            .transpose()?
                            .unwrap_or(64);
                        Ok(FormRule::max_length(max, message))
                    }
                    other => Err(format!("unknown form rule `{other}`")),
                }
            }
            _ => Err("rule: expected string or object".into()),
        })
        .collect()
}

fn rule_from_ident(s: &str) -> Result<FormRule, String> {
    match s {
        "required" => Ok(FormRule::required("required")),
        "digits" => Ok(FormRule::digits("digits only")),
        other => Err(format!("unknown form rule `{other}`")),
    }
}

fn build_widget(
    builder: &mut FlexBuilder<'_>,
    names: &mut HashMap<String, WidgetId>,
    w: &AstWidget,
) -> Result<(), DslError> {
    let kind = w.kind.as_str();
    match kind {
        "Text" => {
            let content = first_string_arg_or(w, "")?;
            let theme = builder.theme();
            let color = prop_color(&w.props, "fg", theme)
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_color(&w.props, "color", theme).map_err(|m| err_at(w.line, w.column, m))?);
            let size = prop_f32(&w.props, "size").map_err(|m| err_at(w.line, w.column, m))?;
            let id = match (color, size) {
                (Some(c), Some(s)) => builder.text_styled(content, c, s),
                (Some(c), None) => builder.text_styled(content, c, theme.typography.body),
                (None, Some(s)) => builder.text_styled(content, theme.colors.text_primary, s),
                (None, None) => builder.text(content),
            };
            register(names, &w.name, id);
        }
        "Button" => {
            let label = first_string_arg_or(w, "")?;
            let theme = builder.theme();
            let bg = prop_color(&w.props, "bg", theme).map_err(|m| err_at(w.line, w.column, m))?;
            let fg = prop_color(&w.props, "fg", theme).map_err(|m| err_at(w.line, w.column, m))?;
            let id = match (bg, fg) {
                (Some(bg), Some(fg)) => builder.button_styled(label, bg, fg),
                (Some(bg), None) => builder.button_styled(label, bg, theme.colors.button_fg),
                (None, Some(fg)) => builder.button_styled(label, theme.colors.button_bg, fg),
                (None, None) => builder.button(label),
            };
            register(names, &w.name, id);
        }
        "Icon" => {
            let name = first_string_arg_or(w, "blank")?;
            let size = prop_f32(&w.props, "size")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(18.0);
            let theme = builder.theme();
            let color = prop_color(&w.props, "color", theme)
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_color(&w.props, "fg", theme).map_err(|m| err_at(w.line, w.column, m))?)
                .unwrap_or(theme.colors.text_secondary);
            let id = builder.icon_styled(IconName::from_name(&name), size, color);
            register(names, &w.name, id);
        }
        "Spacer" => {
            let width = prop_f32(&w.props, "width")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_f32(v, "width").ok()))
                .unwrap_or(12.0);
            let height = prop_f32(&w.props, "height")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_f32(v, "height").ok()))
                .unwrap_or(12.0);
            let id = builder.spacer(width, height);
            register(names, &w.name, id);
        }
        "Separator" => {
            let vertical = prop_bool(&w.props, "vertical")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(false);
            let length = prop_f32(&w.props, "length")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(200.0);
            let id = if vertical {
                builder.separator_vertical(length)
            } else {
                builder.separator_styled(1.0, length, true, Color::rgb(0x55, 0x55, 0x5a))
            };
            register(names, &w.name, id);
        }
        "Column" => {
            let mut style = crate::ColumnStyle::from_theme(builder.theme());
            map_err(w, apply_column_style(&mut style, &w.props, builder.theme()))?;
            let children = w.children.clone();
            let mut err = None;
            let id = builder.column(style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Row" => {
            let mut style = crate::RowStyle::from_theme(builder.theme());
            map_err(w, apply_row_style(&mut style, &w.props, builder.theme()))?;
            let children = w.children.clone();
            let mut err = None;
            let id = builder.row(style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Toolbar" => {
            let mut style = crate::ToolbarStyle::from_theme(builder.theme());
            map_err(w, apply_toolbar_style(&mut style, &w.props, builder.theme()))?;
            let children = w.children.clone();
            let mut err = None;
            let id = builder.toolbar(style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Panel" => {
            let title = first_string_arg_or(w, "")?;
            let mut style = PanelStyle::from_theme(builder.theme());
            if let Some(v) = take_prop(&w.props, "width") {
                style.width = as_f32(v, "width").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "height") {
                style.height = as_f32(v, "height").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "padding") {
                style.padding = as_f32(v, "padding").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "gap") {
                style.gap = as_f32(v, "gap").map_err(|m| err_at(w.line, w.column, m))?;
            }
            let children = w.children.clone();
            let mut err = None;
            let id = builder.panel(title, style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "StatusBar" => {
            let mut style = crate::StatusBarStyle::from_theme(builder.theme());
            map_err(w, apply_status_bar_style(&mut style, &w.props, builder.theme()))?;
            let children = w.children.clone();
            let mut err = None;
            let id = builder.status_bar(style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Splitter" => {
            let mut style = SplitterStyle::from_theme(builder.theme());
            if let Some(v) = take_prop(&w.props, "ratio") {
                style.ratio = as_f32(v, "ratio").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "width") {
                style.width = as_f32(v, "width").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "height") {
                style.height = as_f32(v, "height").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if w.children.len() != 2 {
                return Err(err_at(
                    w.line,
                    w.column,
                    "Splitter requires exactly two child widgets",
                ));
            }
            let a = w.children[0].clone();
            let b = w.children[1].clone();
            // splitter invokes panes sequentially; share names via raw pointer.
            let names_ptr: *mut HashMap<String, WidgetId> = names;
            let mut err_a = None;
            let mut err_b = None;
            let id = builder.splitter(
                style,
                |inner| {
                    let names = unsafe { &mut *names_ptr };
                    if let Err(e) = build_widget(inner, names, &a) {
                        err_a = Some(e);
                    }
                },
                |inner| {
                    let names = unsafe { &mut *names_ptr };
                    if let Err(e) = build_widget(inner, names, &b) {
                        err_b = Some(e);
                    }
                },
            );
            if let Some(e) = err_a.or(err_b) {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "ScrollArea" => {
            let mut style = ScrollAreaStyle::from_theme(builder.theme());
            if let Some(v) = take_prop(&w.props, "width") {
                style.width = as_f32(v, "width").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "height") {
                style.height = as_f32(v, "height").map_err(|m| err_at(w.line, w.column, m))?;
            }
            let children = w.children.clone();
            let mut err = None;
            let id = builder.scroll_area(style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }

        "TextInput" => {
            let placeholder = first_string_arg_or(w, "")?;
            let value = prop_string(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let mut style = TextInputStyle::from_theme(builder.theme());
            if let Some(v) = take_prop(&w.props, "width") {
                style.width = as_f32(v, "width").map_err(|m| err_at(w.line, w.column, m))?;
            }
            let id = builder.text_input_styled(value, placeholder, style);
            register(names, &w.name, id);
        }
        "Checkbox" | "CheckBox" => {
            let label = first_string_arg_or(w, "")?;
            let checked = prop_bool(&w.props, "checked")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_bool(v, "checked").ok()))
                .unwrap_or(false);
            let id = builder.checkbox_styled(label, checked, CheckboxStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Radio" => {
            let group = prop_usize(&w.props, "group")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_usize(v, "group").ok()))
                .unwrap_or(0) as u64;
            let label = if w.args.len() >= 2 {
                as_string(&w.args[1], "label").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                first_string_arg_or(w, "")?
            };
            let selected = prop_bool(&w.props, "selected")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(false);
            let id = builder.radio_styled(group, label, selected, RadioStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Switch" => {
            let label = first_string_arg_or(w, "")?;
            let on = prop_bool(&w.props, "on")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_bool(&w.props, "checked").map_err(|m| err_at(w.line, w.column, m))?)
                .unwrap_or(false);
            let id = builder.switch_styled(label, on, SwitchStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "List" | "VirtualList" => {
            let items = if let Some(v) = w.args.first() {
                as_string_list(v, "items").map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "items") {
                as_string_list(v, "items").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let style = ListStyle::from_theme(builder.theme());
            let id = if kind == "VirtualList" {
                builder.virtual_list(items, style)
            } else {
                builder.list(items, style)
            };
            register(names, &w.name, id);
        }
        "Tree" => {
            let roots = if let Some(v) = w.args.first() {
                parse_tree_nodes(v).map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "nodes") {
                parse_tree_nodes(v).map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let id = builder.tree(roots, TreeStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Table" | "VirtualTable" => {
            let columns = take_prop(&w.props, "columns")
                .or(w.args.first())
                .map(parse_table_columns)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let rows = take_prop(&w.props, "rows")
                .or(w.args.get(1))
                .map(parse_table_rows)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let style = TableStyle::from_theme(builder.theme());
            let id = if kind == "VirtualTable" {
                builder.virtual_table(columns, rows, style)
            } else {
                builder.table(columns, rows, style)
            };
            register(names, &w.name, id);
        }
        "Menu" => {
            let title = first_string_arg_or(w, "Menu")?;
            let items = take_prop(&w.props, "items")
                .or(w.args.get(1))
                .map(parse_menu_items)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.menu(title, items, MenuStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "ContextMenu" => {
            let hint = first_string_arg_or(w, "")?;
            let items = take_prop(&w.props, "items")
                .map(parse_menu_items)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let children = w.children.clone();
            let mut err = None;
            let id = builder.context_menu(hint, items, ContextMenuStyle::from_theme(builder.theme()), |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Tabs" => {
            let mut style = TabsStyle::from_theme(builder.theme());
            if let Some(v) = take_prop(&w.props, "scrollable") {
                style.scrollable = as_bool(v, "scrollable").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "strip_at_bottom") {
                style.strip_at_bottom = as_bool(v, "strip_at_bottom").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "width") {
                style.width = as_f32(v, "width").map_err(|m| err_at(w.line, w.column, m))?;
            }
            if let Some(v) = take_prop(&w.props, "height") {
                style.height = as_f32(v, "height").map_err(|m| err_at(w.line, w.column, m))?;
            }
            let pages: Vec<(String, Vec<AstWidget>)> = w
                .children
                .iter()
                .map(|c| {
                    if c.kind != "Tab" && c.kind != "Page" {
                        return Err(err_at(
                            c.line,
                            c.column,
                            "Tabs children must be Tab(\"title\") { ... }",
                        ));
                    }
                    let title = c
                        .args
                        .first()
                        .map(|v| as_string(v, "title"))
                        .transpose()
                        .map_err(|m| err_at(c.line, c.column, m))?
                        .unwrap_or_else(|| "Tab".into());
                    Ok((title, c.children.clone()))
                })
                .collect::<Result<_, _>>()?;
            let mut err = None;
            let id = builder.tabs(style, |tb| {
                for (title, kids) in &pages {
                    tb.page(title.clone(), |inner| {
                        if err.is_none() {
                            if let Err(e) = build_children(inner, names, kids) {
                                err = Some(e);
                            }
                        }
                    });
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Select" => {
            let options = if let Some(v) = w.args.first() {
                as_string_list(v, "options").map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "options") {
                as_string_list(v, "options").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let selected = prop_usize(&w.props, "selected")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let placeholder = prop_string(&w.props, "placeholder")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.select(options, selected, placeholder, SelectStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Slider" => {
            let min = prop_f32(&w.props, "min")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_f32(v, "min").ok()))
                .unwrap_or(0.0);
            let max = prop_f32(&w.props, "max")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_f32(v, "max").ok()))
                .unwrap_or(1.0);
            let value = prop_f32(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(2).and_then(|v| as_f32(v, "value").ok()))
                .unwrap_or(min);
            let id = builder.slider(min, max, value, SliderStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Tag" => {
            let label = first_string_arg_or(w, "")?;
            let closable = prop_bool(&w.props, "closable")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(false);
            let id = builder.tag_styled(label, closable, TagStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Badge" | "BadgeCount" => {
            let count = prop_usize(&w.props, "count")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_usize(v, "count").ok()))
                .unwrap_or(0) as u32;
            let id = if w.children.is_empty() {
                builder.badge_count(count)
            } else {
                let children = w.children.clone();
                let mut err = None;
                let id = builder.badge_host(
                    BadgeKind::Count(count),
                    BadgeStyle::from_theme(builder.theme()),
                    |inner| {
                        if let Err(e) = build_children(inner, names, &children) {
                            err = Some(e);
                        }
                    },
                );
                if let Some(e) = err {
                    return Err(e);
                }
                id
            };
            register(names, &w.name, id);
        }
        "BadgeDot" => {
            let id = if w.children.is_empty() {
                builder.badge_dot()
            } else {
                let children = w.children.clone();
                let mut err = None;
                let id = builder.badge_host(BadgeKind::Dot, BadgeStyle::from_theme(builder.theme()), |inner| {
                    if let Err(e) = build_children(inner, names, &children) {
                        err = Some(e);
                    }
                });
                if let Some(e) = err {
                    return Err(e);
                }
                id
            };
            register(names, &w.name, id);
        }
        "Tooltip" => {
            let text = first_string_arg_or(w, "")?;
            let children = w.children.clone();
            let mut err = None;
            let id = builder.tooltip(text, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Progress" => {
            let value = prop_f32(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_f32(v, "value").ok()))
                .unwrap_or(0.0);
            let indeterminate = prop_bool(&w.props, "indeterminate")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(false);
            let id = builder.progress_styled(value, indeterminate, ProgressStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Alert" => {
            let tone_s = w
                .args
                .first()
                .map(|v| as_ident(v, "tone"))
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_string(&w.props, "tone").map_err(|m| err_at(w.line, w.column, m))?)
                .unwrap_or_else(|| "info".into());
            let tone = parse_alert_tone(&tone_s).map_err(|m| err_at(w.line, w.column, m))?;
            let message = if w.args.len() >= 2 {
                as_string(&w.args[1], "message").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                prop_string(&w.props, "message")
                    .map_err(|m| err_at(w.line, w.column, m))?
                    .unwrap_or_default()
            };
            let dismissible = prop_bool(&w.props, "dismissible")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(true);
            let id = builder.alert_styled(tone, message, dismissible, AlertStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }

        "TreeSelect" => {
            let roots = take_prop(&w.props, "nodes")
                .or(w.args.first())
                .map(parse_tree_nodes)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let placeholder = prop_string(&w.props, "placeholder")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.tree_select(roots, None, placeholder, TreeSelectStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Form" => {
            let style = FormStyle::from_theme(builder.theme());
            let fields: Vec<AstWidget> = w.children.clone();
            let mut err = None;
            let id = builder.form(style, |fb| {
                for field in &fields {
                    if field.kind != "Field" {
                        err = Some(err_at(
                            field.line,
                            field.column,
                            "Form children must be Field(\"key\", \"Label\") { control }",
                        ));
                        break;
                    }
                    let key = field
                        .args
                        .first()
                        .and_then(|v| as_string(v, "key").ok())
                        .unwrap_or_else(|| "field".into());
                    let label = field
                        .args
                        .get(1)
                        .and_then(|v| as_string(v, "label").ok())
                        .unwrap_or_else(|| key.clone());
                    let rules = take_prop(&field.props, "rules")
                        .or(field.args.get(2))
                        .map(parse_form_rules)
                        .transpose();
                    let rules = match rules {
                        Ok(r) => r.unwrap_or_default(),
                        Err(m) => {
                            err = Some(err_at(field.line, field.column, m));
                            break;
                        }
                    };
                    let kids = field.children.clone();
                    fb.field(key, label, rules, |inner| {
                        if err.is_none() {
                            if let Err(e) = build_children(inner, names, &kids) {
                                err = Some(e);
                            }
                        }
                    });
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "AlarmList" => {
            let entries = take_prop(&w.props, "entries")
                .or(w.args.first())
                .map(parse_alarm_entries)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.alarm_list(entries, AlarmListStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Gauge" => {
            let value = prop_f32(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_f32(v, "value").ok()))
                .unwrap_or(0.0);
            let min = prop_f32(&w.props, "min")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(0.0);
            let max = prop_f32(&w.props, "max")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(100.0);
            let unit = prop_string(&w.props, "unit")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let label = prop_string(&w.props, "label")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.gauge(value, min, max, unit, label, GaugeStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Trend" => {
            let points = take_prop(&w.props, "points")
                .or(w.args.first())
                .map(|v| as_number_list(v, "points"))
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.trend(points, TrendStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Chart" => {
            let tiles = parse_chart_tiles(builder.theme(), take_prop(&w.props, "tiles").or(w.args.first()))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let mut style = ChartStyle::from_theme(builder.theme());
            style.show_plot_coordinates = prop_bool(&w.props, "show_plot_coordinates")
                .map_err(|m| err_at(w.line,w.column,m))?.unwrap_or(false);
            let id = builder.chart(tiles, style);
            register(names, &w.name, id);
        }
        "TextArea" => {
            let value = prop_string(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let placeholder = first_string_arg_or(w, "")?;
            let line_numbers = prop_bool(&w.props, "line_numbers")
                .map_err(|m| err_at(w.line, w.column, m))?.unwrap_or(false);
            let id = builder.text_area(value, placeholder, TextAreaStyle::from_theme(builder.theme()));
            builder.ui.set_text_area_line_numbers(id, line_numbers);
            register(names, &w.name, id);
        }
        "InputNumber" => {
            let value = prop_f32(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_f32(v, "value").ok()))
                .unwrap_or(0.0) as f64;
            let min = prop_f32(&w.props, "min")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(0.0) as f64;
            let max = prop_f32(&w.props, "max")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(100.0) as f64;
            let step = prop_f32(&w.props, "step")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(1.0) as f64;
            let id = builder.input_number(value, min, max, step, InputNumberStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Spin" => {
            let spinning = prop_bool(&w.props, "spinning")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_bool(v, "spinning").ok()))
                .unwrap_or(true);
            let children = w.children.clone();
            let mut err = None;
            let id = builder.spin(spinning, SpinStyle::from_theme(builder.theme()), |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Empty" => {
            let desc = first_string_arg_or(w, "No data")?;
            let id = builder.empty(desc, EmptyStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Collapse" => {
            let panels = parse_collapse_panels(take_prop(&w.props, "panels").or(w.args.first()))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.collapse(panels, CollapseStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Descriptions" => {
            let items =
                parse_description_items(take_prop(&w.props, "items").or(w.args.first()))
                    .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.descriptions(items, DescriptionsStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Card" => {
            let title = first_string_arg_or(w, "")?;
            let body = prop_string(&w.props, "body")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_string(v, "body").ok()))
                .unwrap_or_default();
            let id = builder.card(title, body, CardStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Segmented" => {
            let options = if let Some(v) = w.args.first() {
                as_string_list(v, "options").map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "options") {
                as_string_list(v, "options").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let selected = prop_usize(&w.props, "selected")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(0);
            let id = builder.segmented(options, selected, SegmentedStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "ButtonGroup" => {
            let labels = if let Some(v) = w.args.first() {
                as_string_list(v, "labels").map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "labels") {
                as_string_list(v, "labels").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let selected = prop_usize(&w.props, "selected")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.button_group(labels, selected, ButtonGroupStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Link" => {
            let label = first_string_arg_or(w, "")?;
            let id = builder.link(label);
            register(names, &w.name, id);
        }
        "Space" => {
            let mut style = SpaceStyle::from_theme(builder.theme());
            if let Some(v) = take_prop(&w.props, "direction") {
                let d = as_ident(v, "direction").map_err(|m| err_at(w.line, w.column, m))?;
                style.direction = match d.as_str() {
                    "horizontal" | "row" => SpaceDirection::Horizontal,
                    "vertical" | "column" => SpaceDirection::Vertical,
                    other => {
                        return Err(err_at(
                            w.line,
                            w.column,
                            format!("Space direction: expected horizontal|vertical, got {other}"),
                        ));
                    }
                };
            }
            if let Some(v) = take_prop(&w.props, "gap") {
                style.gap = as_f32(v, "gap").map_err(|m| err_at(w.line, w.column, m))?;
            }
            let children = w.children.clone();
            let mut err = None;
            let id = builder.space(style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }

        "Steps" => {
            let items = parse_step_items(take_prop(&w.props, "items").or(w.args.first()))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let current = prop_usize(&w.props, "current")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(0);
            let id = builder.steps(items, current, StepsStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Statistic" => {
            let title = first_string_arg_or(w, "")?;
            let value = prop_string(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_string(v, "value").ok()))
                .unwrap_or_default();
            let id = builder.statistic(title, value, StatisticStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Result" => {
            let status_s = w
                .args
                .first()
                .map(|v| as_ident(v, "status"))
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_string(&w.props, "status").map_err(|m| err_at(w.line, w.column, m))?)
                .unwrap_or_else(|| "info".into());
            let status = match status_s.as_str() {
                "info" => ResultStatus::Info,
                "success" => ResultStatus::Success,
                "warning" => ResultStatus::Warning,
                "error" => ResultStatus::Error,
                other => {
                    return Err(err_at(
                        w.line,
                        w.column,
                        format!("unknown Result status `{other}`"),
                    ));
                }
            };
            let title = if w.args.len() >= 2 {
                as_string(&w.args[1], "title").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                prop_string(&w.props, "title")
                    .map_err(|m| err_at(w.line, w.column, m))?
                    .unwrap_or_default()
            };
            let subtitle = prop_string(&w.props, "subtitle")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let primary = prop_string(&w.props, "primary")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let secondary = prop_string(&w.props, "secondary")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.result(
                status,
                title,
                subtitle,
                primary,
                secondary,
                ResultStyle::from_theme(builder.theme()),
            );
            register(names, &w.name, id);
        }
        "Popover" => {
            let trigger = first_string_arg_or(w, "…")?;
            let body = prop_string(&w.props, "body")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_string(v, "body").ok()))
                .unwrap_or_default();
            let title = prop_string(&w.props, "title")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.popover_titled(trigger, title, body, PopoverStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Popconfirm" => {
            let trigger = first_string_arg_or(w, "…")?;
            let message = prop_string(&w.props, "message")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_string(v, "message").ok()))
                .unwrap_or_default();
            let id = builder.popconfirm(trigger, message, PopconfirmStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Upload" => {
            let placeholder = first_string_arg_or(w, "Select file")?;
            let path = prop_string(&w.props, "path")
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.upload_labeled(
                placeholder,
                "Browse",
                "Clear",
                path,
                UploadStyle::from_theme(builder.theme()),
            );
            register(names, &w.name, id);
        }
        "ColorPicker" => {
            let value = w
                .args
                .first()
                .map(|v| as_color(v, "value", builder.theme()))
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_color(&w.props, "value", builder.theme()).map_err(|m| err_at(w.line, w.column, m))?)
                .unwrap_or(Color::rgb(0x3b, 0x82, 0xf6));
            let id = builder.color_picker(value, ColorPickerStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Autocomplete" => {
            let options = if let Some(v) = w.args.first() {
                as_string_list(v, "options").map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "options") {
                as_string_list(v, "options").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let value = prop_string(&w.props, "value")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let placeholder = prop_string(&w.props, "placeholder")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.autocomplete_placeholder(
                options,
                value,
                placeholder,
                AutocompleteStyle::from_theme(builder.theme()),
            );
            register(names, &w.name, id);
        }
        "Grid" => {
            let columns = prop_usize(&w.props, "columns")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_usize(v, "columns").ok()))
                .unwrap_or(2);
            let style = GridStyle::from_theme(builder.theme());
            let children = w.children.clone();
            let mut err = None;
            let id = builder.grid(columns, style, |inner| {
                if let Err(e) = build_children(inner, names, &children) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            register(names, &w.name, id);
        }
        "Timeline" => {
            let items = parse_timeline_items(take_prop(&w.props, "items").or(w.args.first()))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.timeline(items, TimelineStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Calendar" => {
            let year = prop_f32(&w.props, "year")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_f32(v, "year").ok()))
                .unwrap_or(2026.0) as i32;
            let month = prop_f32(&w.props, "month")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_f32(v, "month").ok()))
                .unwrap_or(10.0) as u8;
            let day = prop_f32(&w.props, "day")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or(prop_f32(&w.props, "selected_day").map_err(|m| err_at(w.line, w.column, m))?)
                .or_else(|| w.args.get(2).and_then(|v| as_f32(v, "day").ok()))
                .unwrap_or(1.0) as u8;
            let id = builder.calendar(year, month, day, CalendarStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "ImagePlaceholder" => {
            let caption = first_string_arg_or(w, "")?;
            let id = builder.image_placeholder(caption, ImageStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "ImageRgba" => {
            return Err(err_at(
                w.line,
                w.column,
                "ImageRgba pixel buffers are Rust-only; use ImagePlaceholder in .ngui",
            ));
        }
        "Breadcrumb" => {
            let items = if let Some(v) = w.args.first() {
                as_string_list(v, "items").map_err(|m| err_at(w.line, w.column, m))?
            } else if let Some(v) = take_prop(&w.props, "items") {
                as_string_list(v, "items").map_err(|m| err_at(w.line, w.column, m))?
            } else {
                Vec::new()
            };
            let id = builder.breadcrumb(items, BreadcrumbStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Pagination" => {
            let page = prop_usize(&w.props, "page")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.first().and_then(|v| as_usize(v, "page").ok()))
                .unwrap_or(1);
            let page_size = prop_usize(&w.props, "page_size")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(1).and_then(|v| as_usize(v, "page_size").ok()))
                .unwrap_or(10);
            let total = prop_usize(&w.props, "total")
                .map_err(|m| err_at(w.line, w.column, m))?
                .or_else(|| w.args.get(2).and_then(|v| as_usize(v, "total").ok()))
                .unwrap_or(0);
            let id = builder.pagination(page, page_size, total, PaginationStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "DateTime" => {
            let year = prop_f32(&w.props, "year")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(2026.0) as i32;
            let month = prop_f32(&w.props, "month")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(1.0) as u8;
            let day = prop_f32(&w.props, "day")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(1.0) as u8;
            let hour = prop_f32(&w.props, "hour")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(0.0) as u8;
            let minute = prop_f32(&w.props, "minute")
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or(0.0) as u8;
            let value = DateTimeValue::new(year, month, day, hour, minute);
            let id = builder.datetime(value, DateTimeStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Transfer" => {
            let source = parse_transfer_items(take_prop(&w.props, "source"))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let target = parse_transfer_items(take_prop(&w.props, "target"))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.transfer(source, target, TransferStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "PropertyGrid" => {
            let rows = parse_property_rows(take_prop(&w.props, "rows").or(w.args.first()))
                .map_err(|m| err_at(w.line, w.column, m))?;
            let id = builder.property_grid(rows, PropertyGridStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "TreeTable" => {
            let columns = take_prop(&w.props, "columns")
                .map(parse_table_columns)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let roots = take_prop(&w.props, "nodes")
                .map(parse_tree_table_nodes)
                .transpose()
                .map_err(|m| err_at(w.line, w.column, m))?
                .unwrap_or_default();
            let id = builder.tree_table(columns, roots, TreeTableStyle::from_theme(builder.theme()));
            register(names, &w.name, id);
        }
        "Modal" | "Drawer" | "Notify" => {
            return Err(err_at(
                w.line,
                w.column,
                format!(
                    "{} must be a document-level overlay after the root Column",
                    w.kind
                ),
            ));
        }
        "Tab" | "Page" | "Field" => {
            return Err(err_at(
                w.line,
                w.column,
                format!("{} is only valid inside Tabs/Form", w.kind),
            ));
        }
        other => {
            return Err(err_at(
                w.line,
                w.column,
                format!("unknown widget `{other}`"),
            ));
        }
    }
    Ok(())
}

fn parse_chart_tiles(theme: &Theme, v: Option<&AstValue>) -> Result<Vec<ChartTile>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("chart tiles: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        let AstValue::Object(fields) = item else {
            return Err("chart tile: expected object".into());
        };
        let title = fields
            .iter()
            .find(|(k, _)| k == "title")
            .map(|(_, v)| as_string(v, "title"))
            .transpose()?
            .unwrap_or_default();
        let kind = fields
            .iter()
            .find(|(k, _)| k == "kind")
            .map(|(_, v)| as_ident(v, "kind"))
            .transpose()?
            .unwrap_or_else(|| "gauge".into());
        match kind.as_str() {
            "gauge" => {
                let value = fields
                    .iter()
                    .find(|(k, _)| k == "value")
                    .map(|(_, v)| as_f32(v, "value"))
                    .transpose()?
                    .unwrap_or(0.0);
                let min = fields
                    .iter()
                    .find(|(k, _)| k == "min")
                    .map(|(_, v)| as_f32(v, "min"))
                    .transpose()?
                    .unwrap_or(0.0);
                let max = fields
                    .iter()
                    .find(|(k, _)| k == "max")
                    .map(|(_, v)| as_f32(v, "max"))
                    .transpose()?
                    .unwrap_or(100.0);
                let unit = fields
                    .iter()
                    .find(|(k, _)| k == "unit")
                    .map(|(_, v)| as_string(v, "unit"))
                    .transpose()?
                    .unwrap_or_default();
                let label = fields
                    .iter()
                    .find(|(k, _)| k == "label")
                    .map(|(_, v)| as_string(v, "label"))
                    .transpose()?
                    .unwrap_or_default();
                out.push(ChartTile::gauge(
                    title,
                    value,
                    min,
                    max,
                    unit,
                    label,
                    GaugeStyle::from_theme(theme),
                ));
            }
            "trend" => {
                let points = fields
                    .iter()
                    .find(|(k, _)| k == "points")
                    .map(|(_, v)| as_number_list(v, "points"))
                    .transpose()?
                    .unwrap_or_default();
                out.push(ChartTile::trend(title, points, TrendStyle::from_theme(theme)));
            }
            other => return Err(format!("unknown chart kind `{other}`")),
        }
    }
    Ok(out)
}

fn parse_collapse_panels(v: Option<&AstValue>) -> Result<Vec<CollapsePanel>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("collapse panels: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        match item {
            AstValue::Object(fields) => {
                let title = fields
                    .iter()
                    .find(|(k, _)| k == "title")
                    .map(|(_, v)| as_string(v, "title"))
                    .transpose()?
                    .unwrap_or_default();
                let body = fields
                    .iter()
                    .find(|(k, _)| k == "body")
                    .map(|(_, v)| as_string(v, "body"))
                    .transpose()?
                    .unwrap_or_default();
                let open = fields
                    .iter()
                    .find(|(k, _)| k == "open")
                    .map(|(_, v)| as_bool(v, "open"))
                    .transpose()?
                    .unwrap_or(false);
                out.push(CollapsePanel::new(title, body, open));
            }
            AstValue::List(pair) if pair.len() >= 2 => {
                out.push(CollapsePanel::new(
                    as_string(&pair[0], "title")?,
                    as_string(&pair[1], "body")?,
                    pair.get(2)
                        .map(|v| as_bool(v, "open"))
                        .transpose()?
                        .unwrap_or(false),
                ));
            }
            _ => return Err("collapse panel: expected object or [title,body,open?]".into()),
        }
    }
    Ok(out)
}

fn parse_description_items(v: Option<&AstValue>) -> Result<Vec<DescriptionItem>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("descriptions: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        match item {
            AstValue::Object(fields) => {
                let label = fields
                    .iter()
                    .find(|(k, _)| k == "label" || k == "key")
                    .map(|(_, v)| as_string(v, "label"))
                    .transpose()?
                    .unwrap_or_default();
                let value = fields
                    .iter()
                    .find(|(k, _)| k == "value")
                    .map(|(_, v)| as_string(v, "value"))
                    .transpose()?
                    .unwrap_or_default();
                out.push(DescriptionItem::new(label, value));
            }
            AstValue::List(pair) if pair.len() >= 2 => {
                out.push(DescriptionItem::new(
                    as_string(&pair[0], "label")?,
                    as_string(&pair[1], "value")?,
                ));
            }
            _ => return Err("description item: expected {label,value} or [label,value]".into()),
        }
    }
    Ok(out)
}

fn parse_step_items(v: Option<&AstValue>) -> Result<Vec<StepItem>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("steps: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        match item {
            AstValue::String(s) => out.push(StepItem::new(s.clone())),
            AstValue::Object(fields) => {
                let title = fields
                    .iter()
                    .find(|(k, _)| k == "title")
                    .map(|(_, v)| as_string(v, "title"))
                    .transpose()?
                    .unwrap_or_default();
                if let Some((_, d)) = fields.iter().find(|(k, _)| k == "description") {
                    out.push(StepItem::with_description(
                        title,
                        as_string(d, "description")?,
                    ));
                } else {
                    out.push(StepItem::new(title));
                }
            }
            _ => return Err("step: expected string or {title,description?}".into()),
        }
    }
    Ok(out)
}

fn parse_timeline_items(v: Option<&AstValue>) -> Result<Vec<TimelineItem>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("timeline: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        match item {
            AstValue::Object(fields) => {
                let time = fields
                    .iter()
                    .find(|(k, _)| k == "time")
                    .map(|(_, v)| as_string(v, "time"))
                    .transpose()?
                    .unwrap_or_default();
                let content = fields
                    .iter()
                    .find(|(k, _)| k == "content")
                    .map(|(_, v)| as_string(v, "content"))
                    .transpose()?
                    .unwrap_or_default();
                out.push(TimelineItem::new(time, content));
            }
            AstValue::List(pair) if pair.len() >= 2 => {
                out.push(TimelineItem::new(
                    as_string(&pair[0], "time")?,
                    as_string(&pair[1], "content")?,
                ));
            }
            _ => return Err("timeline item: expected {time,content} or [time,content]".into()),
        }
    }
    Ok(out)
}

fn parse_transfer_items(v: Option<&AstValue>) -> Result<Vec<TransferItem>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("transfer: expected list".into()),
    };
    let mut out = Vec::new();
    for (i, item) in list.iter().enumerate() {
        match item {
            AstValue::String(s) => out.push(TransferItem::new(i as u64, s.clone())),
            AstValue::Object(fields) => {
                let id = fields
                    .iter()
                    .find(|(k, _)| k == "id")
                    .map(|(_, v)| as_u64(v, "id"))
                    .transpose()?
                    .unwrap_or(i as u64);
                let label = fields
                    .iter()
                    .find(|(k, _)| k == "label")
                    .map(|(_, v)| as_string(v, "label"))
                    .transpose()?
                    .unwrap_or_default();
                out.push(TransferItem::new(id, label));
            }
            _ => return Err("transfer item: expected string or {id,label}".into()),
        }
    }
    Ok(out)
}

fn parse_property_rows(v: Option<&AstValue>) -> Result<Vec<PropertyRow>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("property rows: expected list".into()),
    };
    let mut out = Vec::new();
    for item in list {
        let AstValue::Object(fields) = item else {
            return Err("property row: expected object".into());
        };
        let key = fields
            .iter()
            .find(|(k, _)| k == "key")
            .map(|(_, v)| as_string(v, "key"))
            .transpose()?
            .unwrap_or_default();
        let value = if let Some((_, v)) = fields.iter().find(|(k, _)| k == "value") {
            match v {
                AstValue::String(s) => PropertyValue::Text(s.clone()),
                AstValue::Number(n) => PropertyValue::Number(*n as f64),
                AstValue::Bool(b) => PropertyValue::Bool(*b),
                AstValue::Object(vf) => {
                    let options = vf
                        .iter()
                        .find(|(k, _)| k == "options")
                        .map(|(_, v)| as_string_list(v, "options"))
                        .transpose()?
                        .unwrap_or_default();
                    let selected = vf
                        .iter()
                        .find(|(k, _)| k == "selected")
                        .map(|(_, v)| as_usize(v, "selected"))
                        .transpose()?
                        .unwrap_or(0);
                    PropertyValue::Select { options, selected }
                }
                _ => PropertyValue::Text(String::new()),
            }
        } else {
            PropertyValue::Text(String::new())
        };
        out.push(PropertyRow::new(key, value));
    }
    Ok(out)
}

fn parse_tree_table_nodes(v: &AstValue) -> Result<Vec<TreeTableNode>, String> {
    let list = match v {
        AstValue::List(items) => items,
        _ => return Err("tree table: expected list".into()),
    };
    list.iter().map(parse_tree_table_node).collect()
}

fn parse_tree_table_node(v: &AstValue) -> Result<TreeTableNode, String> {
    let AstValue::Object(fields) = v else {
        return Err("tree table node: expected object".into());
    };
    let id = fields
        .iter()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| as_u64(v, "id"))
        .transpose()?
        .unwrap_or(0);
    let label = fields
        .iter()
        .find(|(k, _)| k == "label")
        .map(|(_, v)| as_string(v, "label"))
        .transpose()?
        .unwrap_or_default();
    let cells = fields
        .iter()
        .find(|(k, _)| k == "cells")
        .map(|(_, v)| as_string_list(v, "cells"))
        .transpose()?
        .unwrap_or_default();
    let children = fields
        .iter()
        .find(|(k, _)| k == "children")
        .map(|(_, v)| parse_tree_table_nodes(v))
        .transpose()?
        .unwrap_or_default();
    Ok(TreeTableNode::new(id, label, cells, children))
}
