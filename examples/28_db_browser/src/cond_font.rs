//! Qt 5 QFont project descriptions; retain properties the format editor does not expose.
use crate::cond_style::{Draft, Style};

pub(crate) const POINT_SCALE: f32 = if cfg!(target_os = "macos") {
    1.0
} else {
    96.0 / 72.0
};

pub(crate) struct Description {
    original: String,
    fields: Vec<String>,
}

impl Description {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let fields: Vec<_> = text.trim().split(',').map(str::to_owned).collect();
        if !matches!(fields.len(), 1 | 2 | 9 | 10 | 11) || fields[0].is_empty() {
            return None;
        }
        let fields = match fields.len() {
            1 | 2 => vec![
                fields[0].clone(),
                fields.get(1).cloned().unwrap_or_else(|| "-1".into()),
                "-1".into(),
                "5".into(),
                "50".into(),
                "0".into(),
                "0".into(),
                "0".into(),
                "0".into(),
                "0".into(),
            ],
            9 => vec![
                fields[0].clone(),
                fields[1].clone(),
                "-1".into(),
                fields[2].clone(),
                fields[3].clone(),
                u8::from(number(&fields[4]) != 0).to_string(),
                fields[5].clone(),
                fields[6].clone(),
                fields[7].clone(),
                "0".into(),
            ],
            _ => fields,
        };
        Some(Self {
            original: text.into(),
            fields,
        })
    }

    pub(crate) fn family(&self) -> &str {
        &self.fields[0]
    }
    pub(crate) fn bold(&self) -> bool {
        number(&self.fields[4]).clamp(0, 99) > 57
    }
    pub(crate) fn weight(&self) -> u16 {
        let legacy = number(&self.fields[4]).clamp(0, 99);
        // Qt's legacy-to-OpenType conversion chooses the nearest anchor, preferring lighter ties.
        [
            (0, 100),
            (12, 200),
            (25, 300),
            (50, 400),
            (57, 500),
            (63, 600),
            (75, 700),
            (81, 800),
            (87, 900),
        ]
        .into_iter()
        .min_by_key(|(old, _)| (legacy - *old).abs())
        .unwrap()
        .1
    }
    pub(crate) fn italic(&self) -> bool {
        number(&self.fields[5]) != 0
    }
    pub(crate) fn underline(&self) -> bool {
        number(&self.fields[6]) != 0
    }
    pub(crate) fn strikeout(&self) -> bool {
        number(&self.fields[7]) != 0
    }

    pub(crate) fn pixels(&self) -> Option<f32> {
        let pixels = number(&self.fields[2]);
        if pixels > 0 {
            return Some(pixels as f32);
        }
        self.fields[1]
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| value * POINT_SCALE)
    }

    pub(crate) fn size_text(&self) -> String {
        self.pixels()
            .map(|value| (value / POINT_SCALE).to_string())
            .unwrap_or_default()
    }

    pub(crate) fn size(&self) -> Option<u16> {
        self.pixels()
            .map(|value| (value / POINT_SCALE).round().clamp(1.0, u16::MAX as f32) as u16)
    }

    pub(crate) fn update(&self, draft: &Draft, size: Option<u16>) -> String {
        let mut fields = self.fields.clone();
        let mut changed = false;
        for (index, value, different) in [
            (
                0,
                if draft.family.is_empty() {
                    "Sans Serif".into()
                } else {
                    draft.family.clone()
                },
                draft.family != self.family(),
            ),
            (
                4,
                if draft.bold { "75" } else { "50" }.into(),
                draft.bold != self.bold(),
            ),
            (
                5,
                u8::from(draft.italic).to_string(),
                draft.italic != self.italic(),
            ),
            (
                6,
                u8::from(draft.underline).to_string(),
                draft.underline != self.underline(),
            ),
        ] {
            if different {
                fields[index] = value;
                changed = true;
            }
        }
        if draft.size != self.size_text() {
            fields[1] = size
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-1".into());
            fields[2] = "-1".into();
            changed = true;
        }
        if changed {
            fields.join(",")
        } else {
            self.original.clone()
        }
    }
}

fn number(text: &str) -> i32 {
    text.trim().parse().unwrap_or(0)
}

pub(crate) fn encode(style: &Style) -> Option<String> {
    if let Some(description) = &style.qfont {
        return Some(description.clone());
    }
    if style.family.is_empty()
        && style.size.is_none()
        && !style.bold
        && !style.italic
        && !style.underline
    {
        return None;
    }
    Some(format!(
        "{},{},-1,5,{},{},{},0,0,0",
        if style.family.is_empty() {
            "Sans Serif"
        } else {
            &style.family
        },
        style.size.unwrap_or(13),
        if style.bold { 75 } else { 50 },
        u8::from(style.italic),
        u8::from(style.underline)
    ))
}

pub(crate) fn decode(text: &str) -> Style {
    let mut style = Style {
        qfont: Some(text.into()),
        ..Default::default()
    };
    if let Some(font) = Description::parse(text) {
        style.family = font.family().into();
        style.size = font.size();
        style.bold = font.bold();
        style.italic = font.italic();
        style.underline = font.underline();
    }
    style
}

pub(crate) fn prepare_qa(state: &mut crate::state::AppState) {
    crate::cond_style::prepare_qa(state);
    for (rule, description) in state.cond_rules.iter_mut().zip([
        "Menlo,18.5,-1,5,63,2,1,1,1,0,Oblique",
        "Menlo,-1,25,5,50,0,0,1,1,0",
        "Times New Roman,16,-1,5,50,1,0,1,0,0",
    ]) {
        let mut style = decode(description);
        style.foreground_rgb = rule.style.foreground_rgb;
        style.alignment = rule.style.alignment;
        rule.style = style;
    }
    state.begin_cond_formats().unwrap();
}

pub(crate) fn prepare_weight_qa(state: &mut crate::state::AppState) {
    prepare_qa(state);
    let family = ["Avenir Next", "Avenir", "Helvetica Neue", "DejaVu Sans"]
        .into_iter()
        .find(|family| {
            next_gui_text::system_font_families()
                .iter()
                .any(|name| name == family)
        })
        .unwrap_or("sans-serif");
    for (index, description) in [
        format!("{family},18.5,-1,5,57,2,1,1,0,0"),
        format!("{family},-1,25,5,63,0,0,1,0,0"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut style = decode(&description);
        style.foreground_rgb = state.cond_rules[index].style.foreground_rgb;
        style.alignment = state.cond_rules[index].style.alignment;
        state.cond_rules[index].style = style;
    }
    state.begin_cond_formats().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_weights_follow_qt_conversion_without_reducing_medium_and_demi_to_bold() {
        for (legacy, expected) in [
            (0, 100),
            (12, 200),
            (25, 300),
            (50, 400),
            (57, 500),
            (60, 500),
            (63, 600),
            (69, 600),
            (75, 700),
            (81, 800),
            (87, 900),
            (99, 900),
        ] {
            let text = format!("Avenir Next,18,-1,5,{legacy},0,0,0,0,0");
            let font = Description::parse(&text).unwrap();
            assert_eq!(font.weight(), expected, "{legacy}");
            let style = decode(&text);
            assert_eq!(
                Draft::from(&style).validate().unwrap().qfont.as_deref(),
                Some(text.as_str())
            );
        }
    }

    #[test]
    fn native_qfont_preserves_unedited_fractional_pixel_and_legacy_properties() {
        for text in [
            "Menlo",
            "Menlo,18.5",
            "Menlo,18.5,5,63,1,1,1,1,0",
            " Menlo,18.5,-1,5,63,2,1,1,1,0,Oblique ",
            "Menlo,-1,25,5,57,0,0,1,1,0",
        ] {
            let style = decode(text);
            let mut draft = Draft::from(&style);
            draft.foreground = "#AABBCC".into();
            draft.alignment = 2;
            let saved = draft.validate().unwrap();
            assert_eq!(saved.qfont.as_deref(), Some(text));
            let font = Description::parse(text).unwrap();
            assert_eq!(
                font.pixels(),
                Description::parse(saved.qfont.as_deref().unwrap())
                    .unwrap()
                    .pixels()
            );
            draft.bold = !draft.bold;
            let saved = draft.validate().unwrap();
            let edited = Description::parse(saved.qfont.as_deref().unwrap()).unwrap();
            assert_eq!(edited.bold(), draft.bold);
            assert_eq!(edited.strikeout(), font.strikeout());
            assert_eq!(edited.fields[8], font.fields[8]);
            assert_eq!(edited.pixels(), font.pixels());
            draft.size = "20".into();
            let saved = draft.validate().unwrap();
            assert_eq!(
                Description::parse(saved.qfont.as_deref().unwrap())
                    .unwrap()
                    .pixels(),
                Some(20.0 * POINT_SCALE)
            );
        }
        for text in [
            "",
            ",12",
            "Font,12,1",
            "Font,12,-1,5,50,0,0,0",
            "Font,12,-1,5,50,0,0,0,0,0,x,extra",
        ] {
            assert!(Description::parse(text).is_none());
            assert_eq!(encode(&decode(text)).as_deref(), Some(text));
        }
        let font = Description::parse("Menlo,18.5,-1,5,57,2,1,1,1,0").unwrap();
        assert!(!font.bold() && font.italic() && font.underline() && font.strikeout());
    }
    #[test]
    fn native_font_controls_render_and_save_cancel_without_normalizing_unedited_properties() {
        use crate::browse_filter::tests::{capture, fixture};
        use next_gui_ui::Theme;
        for (prefix, prepare) in [
            ("native-font", prepare_qa as fn(&mut crate::state::AppState)),
            ("weighted-font", prepare_weight_qa),
        ] {
            for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
                for (width, height) in [(1400, 860), (800, 600)] {
                    let mut demo = fixture(width, height, theme);
                    prepare(&mut demo.state);
                    if width == 800 {
                        demo.state.prefs.font_size = 18.0;
                    }
                    demo.state.overlay = None;
                    demo.state.cond_formats = None;
                    demo.do_rebuild();
                    let original = demo.state.cond_rules.clone();
                    let styles = crate::cond_match::styles(&demo.state, demo.ui.font());
                    assert_eq!(
                        styles[1][1].as_ref().unwrap().size,
                        Some(18.5 * POINT_SCALE)
                    );
                    assert!(styles[1][1].as_ref().unwrap().strikeout);
                    assert_eq!(styles[0][1].as_ref().unwrap().size, Some(25.0));
                    capture(&mut demo, &format!("{prefix}-browse-{theme_name}-{width}"));
                    demo.state.begin_cond_formats().unwrap();
                    demo.do_rebuild();
                    assert_eq!(demo.state.cond_style.size, "18.5");
                    capture(&mut demo, &format!("{prefix}-manager-{theme_name}-{width}"));
                    demo.ui
                        .set_text_input(demo.ids.cond_formats.foreground, "#F8D56B");
                    demo.snapshot_inputs();
                    demo.do_rebuild();
                    assert_eq!(demo.state.cond_style.qfont, original[0].style.qfont);
                    capture(&mut demo, &format!("{prefix}-edited-{theme_name}-{width}"));
                    demo.handle_button(demo.ids.ov_btn_cancel);
                    demo.do_rebuild();
                    assert_eq!(demo.state.cond_rules, original);
                    capture(&mut demo, &format!("{prefix}-cancel-{theme_name}-{width}"));
                    demo.state.begin_cond_formats().unwrap();
                    demo.do_rebuild();
                    demo.ui
                        .set_text_input(demo.ids.cond_formats.foreground, "#F8D56B");
                    demo.handle_button(demo.ids.ov_btn_ok);
                    demo.do_rebuild();
                    assert!(demo.state.overlay.is_none(), "{:?}", demo.state.error_msg);
                    assert_eq!(
                        demo.state.cond_rules[0].style.qfont,
                        original[0].style.qfont
                    );
                    assert_eq!(
                        demo.state.cond_rules[0].style.foreground_rgb,
                        Some(0xF8D56B)
                    );
                    assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());
                    assert_eq!(demo.state.browse().result.as_ref().unwrap().rows.len(), 5);
                    capture(&mut demo, &format!("{prefix}-saved-{theme_name}-{width}"));
                }
            }
        }
    }
}
