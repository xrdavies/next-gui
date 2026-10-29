use super::*;

/// Horizontal alignment of one-line table text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TableTextAlign {
    /// Place text at the leading edge.
    #[default]
    Left,
    /// Place text at the trailing edge.
    Right,
    /// Center text horizontally.
    Center,
    /// A single final line stays left aligned rather than stretching its spaces.
    Justify,
}

/// Overrides for one table cell. Selection retains the table's selected colors.
/// Missing fields inherit table typography/colors; copied text is unaffected.
#[derive(Debug, Clone, Default)]
pub struct TableCellStyle {
    /// Optional cell background, overriding the legacy background matrix.
    pub background: Option<Color>,
    /// Optional text foreground.
    pub foreground: Option<Color>,
    /// Font face/stack, including any bold/italic styling and glyph fallbacks.
    pub font: Option<Font>,
    /// Font size in pixels, between1 and4096 inclusive.
    pub size: Option<f32>,
    /// Draw an underline over the text's advance width.
    pub underline: bool,
    /// Horizontal text placement.
    pub alignment: TableTextAlign,
}

impl Ui {
    /// Replace per-cell styles. Reject invalid sizes or out-of-range metadata
    /// atomically. Replacing rows clears styles; omitted cells inherit defaults.
    pub fn set_table_cell_styles(
        &mut self,
        id: WidgetId,
        styles: Vec<Vec<Option<TableCellStyle>>>,
    ) -> bool {
        let Some(Widget {
            kind:
                Kind::Table {
                    rows,
                    columns,
                    cell_styles,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if styles.len() > rows.len()
            || styles.iter().any(|row| {
                row.len() > columns.len()
                    || row.iter().flatten().any(|style| {
                        style.size.is_some_and(|size| {
                            !size.is_finite() || !(1.0..=4096.0).contains(&size)
                        })
                    })
            })
        {
            return false;
        }
        *cell_styles = styles;
        true
    }
}

pub(super) fn text_mask(
    default_font: &Font,
    text: &str,
    style: &TableCellStyle,
    size: f32,
    width: f32,
    height: f32,
) -> AlphaMask {
    if text.is_empty() || width <= 0.0 || height <= 0.0 {
        return AlphaMask::new(0, 0, Vec::<u8>::new());
    }
    let font = style.font.as_ref().unwrap_or(default_font);
    let size = style.size.unwrap_or(size);
    let mut layout = font.layout(text, size, None);
    let x = match style.alignment {
        TableTextAlign::Left | TableTextAlign::Justify => 0.0,
        TableTextAlign::Right => width - layout.width,
        TableTextAlign::Center => (width - layout.width) * 0.5,
    };
    let y = (height - layout.height) * 0.5;
    for glyph in &mut layout.glyphs {
        glyph.x += x;
        glyph.baseline_y += y;
    }
    let mut mask =
        font.rasterize_layout_clipped(&layout, size, width.ceil() as u32, height.ceil() as u32);
    if style.underline && !mask.is_empty() {
        let start = x.max(0.0).ceil() as u32;
        let end = (x + layout.width).min(width).max(0.0).ceil() as u32;
        let top = (y + font.metrics(size).ascent + (size * 0.08).max(1.0)).round() as i32;
        let pixels = Arc::make_mut(&mut mask.pixels);
        for row in top.max(0)..(top + (size / 18.0).ceil() as i32).min(mask.height as i32) {
            for col in start..end.min(mask.width) {
                pixels[row as usize * mask.width as usize + col as usize] = 255;
            }
        }
    }
    AlphaMask::new(mask.width, mask.height, mask.pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink_left(mask: &AlphaMask) -> usize {
        mask.pixels
            .iter()
            .enumerate()
            .filter(|(_, value)| **value > 0)
            .map(|(index, _)| index % mask.width as usize)
            .min()
            .unwrap()
    }

    #[test]
    fn typography_aligns_underlines_and_clips_large_and_long_text() {
        let ui = Ui::new();
        let font = ui.font();
        let mut style = TableCellStyle {
            size: Some(20.0),
            ..Default::default()
        };
        let left = text_mask(font, "Ab", &style, 13.0, 140.0, 40.0);
        style.alignment = TableTextAlign::Center;
        let center = text_mask(font, "Ab", &style, 13.0, 140.0, 40.0);
        style.alignment = TableTextAlign::Right;
        let right = text_mask(font, "Ab", &style, 13.0, 140.0, 40.0);
        assert!(ink_left(&left) < ink_left(&center) && ink_left(&center) < ink_left(&right));
        style.alignment = TableTextAlign::Justify;
        assert_eq!(
            left.pixels,
            text_mask(font, "Ab", &style, 13.0, 140.0, 40.0).pixels
        );
        style.underline = true;
        let underlined = text_mask(font, "Ab", &style, 13.0, 140.0, 40.0);
        assert!(
            underlined
                .pixels
                .iter()
                .map(|value| *value as u64)
                .sum::<u64>()
                > left.pixels.iter().map(|value| *value as u64).sum()
        );
        assert!(underlined.pixels.chunks(140).any(|row| row
            .iter()
            .filter(|value| **value == 255)
            .count()
            >= font.measure_line("Ab", 20.0).0.floor() as usize - 1));
        for alignment in [
            TableTextAlign::Left,
            TableTextAlign::Center,
            TableTextAlign::Right,
        ] {
            style.size = Some(99.0);
            style.alignment = alignment;
            let mask = text_mask(font, &"中文Ag ".repeat(1000), &style, 13.0, 70.0, 28.0);
            assert_eq!(mask.pixels.len(), 70 * 28);
            assert!(mask.pixels.iter().any(|value| *value > 0));
        }
    }

    #[test]
    fn cell_styles_preserve_selection_frozen_geometry_copy_and_atomic_validation() {
        for virtualized in [false, true] {
            let mut ui = Ui::new();
            let mut scene = Scene::new();
            let mut id = WidgetId(0);
            let base = TableStyle {
                width: 300.0,
                height: 180.0,
                row_height: 36.0,
                ..Default::default()
            };
            ui.column(ColumnStyle::default(), |c| {
                let columns = ["First", "Styled", "Last"]
                    .map(|name| TableColumn::new(name, 140.0))
                    .to_vec();
                let rows = (0..30)
                    .map(|row| vec![row.to_string(), "Ab".into(), "Chinese 中文".into()])
                    .collect();
                id = if virtualized {
                    c.virtual_table(columns, rows, base)
                } else {
                    c.table(columns, rows, base)
                };
            });
            let foreground = Color::rgb(0xff, 0xcc, 0x22);
            let background = Color::rgb(0x32, 0x16, 0x48);
            let cell = TableCellStyle {
                foreground: Some(foreground),
                background: Some(background),
                size: Some(20.0),
                underline: true,
                font: Some(ui.font().clone().with_synthetic_style(true, true)),
                alignment: TableTextAlign::Right,
            };
            assert!(ui.set_table_cell_styles(
                id,
                (0..30).map(|_| vec![None, Some(cell.clone())]).collect()
            ));
            for size in [f32::NAN, f32::INFINITY, 0.0, -1.0, 4097.0] {
                let invalid = TableCellStyle {
                    size: Some(size),
                    ..Default::default()
                };
                assert!(!ui.set_table_cell_styles(id, vec![vec![Some(invalid)]]));
            }
            assert!(!ui.set_table_cell_styles(id, vec![vec![None; 4]]));
            assert!(!ui.set_table_cell_styles(id, vec![vec![]; 31]));
            assert!(!ui.set_table_cell_styles(WidgetId(0), vec![]));
            ui.set_table_frozen_cols(id, 1);
            ui.set_table_scroll_offset(id, 40.0, 0.0);
            ui.sync(&mut scene);
            let widget = &ui.widgets[&id];
            let text = scene.get(widget.aux[3 + 1 + 3 + 1]).unwrap();
            let fill = scene.get(widget.aux[3 + 1 + 1]).unwrap();
            assert_eq!(text.fill, foreground);
            assert_eq!(fill.fill, background);
            assert_eq!(text.mask.as_ref().unwrap().pixels.len(), 132 * 36);
            let clip = text.clip.unwrap();
            let host = scene.get(widget.node.unwrap()).unwrap().bounds;
            assert!(clip.x >= host.x + base.padding + 140.0);
            assert!(clip.x + clip.width <= host.x + host.width - base.padding + 0.01);
            ui.set_table_selected_cells(id, Some((0, 0)), Some((1, 1)));
            ui.sync(&mut scene);
            let widget = &ui.widgets[&id];
            assert_eq!(
                scene.get(widget.aux[3 + 1 + 3 + 1]).unwrap().fill,
                base.selected_fg
            );
            assert_eq!(
                scene.get(widget.aux[3 + 1 + 1]).unwrap().fill,
                base.selected_bg
            );
            assert_eq!(ui.table_selected_text(id).as_deref(), Some("Ab"));
            ui.set_table_rows(id, vec![vec!["replacement".into()]]);
            ui.sync(&mut scene);
            let Kind::Table {
                cell_styles,
                cell_bg,
                ..
            } = &ui.widgets[&id].kind
            else {
                panic!()
            };
            assert!(cell_styles.is_empty() && cell_bg.is_empty());
        }
    }
}
