use super::*;

impl Ui {
    /// Fill the remaining main-axis space in a row/column/scroll area, sharing it equally
    /// with other fill children. The child also stretches across the parent.
    /// Fixed children retain their measured sizes. Returns false for an unknown ID.
    pub fn set_flex_fill(&mut self, id: WidgetId, fill: bool) -> bool {
        if !self.widgets.contains_key(&id) {
            return false;
        }
        if fill {
            return self.set_flex_fill_minimum(id, 0.0);
        }
        self.flex_fill.remove(&id);
        true
    }

    /// Fill remaining space while retaining a minimum main-axis size.
    /// A scroll area scrolls when the minimum and fixed controls exceed its viewport.
    /// Invalid values and unknown IDs leave the previous setting unchanged.
    pub fn set_flex_fill_minimum(&mut self, id: WidgetId, minimum: f32) -> bool {
        if !minimum.is_finite() || minimum < 0.0 || !self.widgets.contains_key(&id) {
            return false;
        }
        self.flex_fill.insert(id, minimum);
        true
    }

    pub(super) fn flex_child_measures(
        &self,
        params: &FlexParams,
        children: &[WidgetId],
    ) -> Vec<ChildMeasure> {
        let mut measures: Vec<_> = children.iter().map(|id| self.measure_widget(*id)).collect();
        let count = children
            .iter()
            .filter(|id| self.flex_fill.contains_key(id))
            .count();
        if count == 0 {
            return measures;
        }
        let vertical = params.axis == Axis::Vertical;
        let span = if vertical {
            params.max_height
        } else {
            params.max_width
        };
        let fixed: f32 = children
            .iter()
            .zip(&measures)
            .filter(|(id, _)| !self.flex_fill.contains_key(id))
            .map(|(_, size)| if vertical { size.height } else { size.width })
            .sum();
        let available = (span
            - 2.0 * params.padding
            - params.gap * children.len().saturating_sub(1) as f32
            - fixed)
            .max(0.0);
        let minimum: f32 = children
            .iter()
            .filter_map(|id| self.flex_fill.get(id))
            .sum();
        let extra = ((available - minimum) / count as f32).max(0.0);
        for (id, size) in children.iter().zip(&mut measures) {
            if let Some(minimum) = self.flex_fill.get(id) {
                *size = if vertical {
                    ChildMeasure::new(
                        (params.max_width - 2.0 * params.padding).max(0.0),
                        minimum + extra,
                    )
                } else {
                    ChildMeasure::new(
                        minimum + extra,
                        (params.max_height - 2.0 * params.padding).max(0.0),
                    )
                };
            }
        }
        measures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_area_fills_a_filtered_table_and_retains_minimum_on_resize() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        #[derive(Debug)]
        struct Rows(Arc<AtomicUsize>);
        impl TableDataSource for Rows {
            fn len(&self) -> usize {
                1_000_000
            }
            fn row(&self, index: usize) -> Result<Vec<String>, String> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Ok(vec![index.to_string()])
            }
        }
        let reads = Arc::new(AtomicUsize::new(0));
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut table = WidgetId(0);
        let mut toolbar = WidgetId(0);
        let mut footer = WidgetId(0);
        let scroll = ui.scroll_area(
            ScrollAreaStyle {
                width: 600.0,
                height: 520.0,
                padding: 4.0,
                gap: 6.0,
                ..Default::default()
            },
            |p| {
                toolbar = p.tool_button("Choose table");
                (table, _) = p.filtered_data_table(
                    vec![TableColumn::new("Record", 200.0)],
                    Arc::new(Rows(reads.clone())),
                    &[],
                    TableStyle {
                        width: 592.0,
                        height: 190.0,
                        ..Default::default()
                    },
                    TextInputStyle {
                        height: 26.0,
                        ..Default::default()
                    },
                );
                footer = p.tool_button("Last record");
            },
        );
        let container = ui.filtered_table_container(table).unwrap();
        assert_eq!(ui.filtered_table_container(footer), None);
        assert!(ui.set_flex_fill_minimum(container, 190.0));
        for invalid in [f32::NAN, f32::INFINITY, -1.0] {
            assert!(!ui.set_flex_fill_minimum(container, invalid));
        }
        assert!(!ui.set_flex_fill_minimum(WidgetId(0), 100.0));
        assert_eq!(ui.flex_fill[&container], 190.0);
        ui.set_flex_fill(table, true);
        ui.sync(&mut scene);
        ui.set_table_selected_row(table, Some(999_999));
        ui.reveal_table_selection(table);
        ui.sync(&mut scene);
        let bounds =
            |ui: &Ui, scene: &Scene, id| scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
        for height in [900.0, 180.0, 520.0] {
            if let Kind::ScrollArea { style, .. } = &mut ui.widgets.get_mut(&scroll).unwrap().kind {
                style.height = height;
            }
            ui.sync(&mut scene);
            let grid = bounds(&ui, &scene, table);
            let bar = bounds(&ui, &scene, toolbar);
            let nav = bounds(&ui, &scene, footer);
            let group = bounds(&ui, &scene, container);
            assert_eq!(group.width, 592.0);
            assert_eq!(group.y, bar.y + bar.height + 6.0);
            assert_eq!(grid.y + grid.height + 6.0, nav.y);
            assert!(grid.height >= 160.0);
            if height > 200.0 {
                assert_eq!(ui.scroll_offset(scroll), Some((0.0, 0.0)));
                assert!(
                    (nav.y + nav.height - (height - 4.0)).abs() < 0.01,
                    "one layout pass must fill the new viewport: {nav:?}"
                );
                assert_eq!(bar.y, 4.0);
                assert_eq!(
                    ui.hit_widget(&scene, nav.x + 5.0, nav.y + nav.height / 2.0),
                    Some(footer)
                );
            }
            if height > 200.0 {
                assert_eq!(
                    ui.table_cell_at(&scene, table, grid.x + 20.0, grid.y + grid.height - 15.0),
                    Some((999_999, 0))
                );
            }
            assert!(ui.widgets[&table].aux.len() < 180);
            if height < 200.0 {
                ui.set_scroll_offset(scroll, 0.0, 1_000.0);
                ui.sync(&mut scene);
                assert!(ui.scroll_offset(scroll).unwrap().1 > 0.0);
            }
        }
        assert!(reads.load(Ordering::Relaxed) < 500);
        assert!(ui.set_flex_fill(container, false));
        assert!(!ui.flex_fill.contains_key(&container));
    }

    #[test]
    fn splitter_resizes_a_fill_table_live_and_retains_fixed_controls() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut table = WidgetId(0);
        let mut toolbar = WidgetId(0);
        let split = ui.splitter(
            SplitterStyle {
                width: 600.0,
                height: 500.0,
                axis: Axis::Horizontal,
                ratio: 0.5,
                ..Default::default()
            },
            |p| {
                p.text_area("SELECT * FROM orders", "", TextAreaStyle::default());
            },
            |p| {
                p.column(
                    ColumnStyle {
                        width: 600.0,
                        height: 250.0,
                        padding: 4.0,
                        gap: 6.0,
                        ..Default::default()
                    },
                    |p| {
                        toolbar = p.row(
                            RowStyle {
                                width: 592.0,
                                height: 32.0,
                                padding: 0.0,
                                ..Default::default()
                            },
                            |p| {
                                p.tool_button("Export results");
                            },
                        );
                        table = p.virtual_table(
                            vec![TableColumn::new("id", 100.0)],
                            (0..1000).map(|i| vec![i.to_string()]).collect(),
                            TableStyle::default(),
                        );
                    },
                );
            },
        );
        assert!(ui.set_flex_fill(table, true));
        assert!(!ui.set_flex_fill(WidgetId(0), true));
        ui.set_splitter_min_sizes(split, 80.0, 100.0);
        ui.sync(&mut scene);
        ui.set_table_selected_row(table, Some(999));
        ui.reveal_table_selection(table);
        ui.sync(&mut scene);
        let bounds =
            |ui: &Ui, scene: &Scene, id| scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
        let mut heights = Vec::new();
        for target in [0.2, 0.8, 0.4] {
            let sash = scene
                .get(ui.widgets[&split].chrome_node.unwrap())
                .unwrap()
                .bounds;
            assert!(ui.pointer_pressed(&mut scene, sash.x + 100.0, sash.y + 2.0));
            assert!(ui.pointer_moved(sash.x + 100.0, 500.0 * target));
            ui.pointer_released();
            ui.sync(&mut scene);
            let t = bounds(&ui, &scene, table);
            let bar = bounds(&ui, &scene, toolbar);
            assert_eq!(bar.height, 28.0);
            assert_eq!(t.y, bar.y + bar.height + 6.0);
            assert!((t.y + t.height - 496.0).abs() < 0.01);
            assert_eq!(t.width, 592.0);
            assert_eq!(
                ui.table_cell_at(&scene, table, t.x + 20.0, t.y + t.height - 15.0),
                Some((999, 0))
            );
            assert!(ui.widgets[&table].aux.len() < 150);
            heights.push(t.height);
        }
        assert!(heights[0] > heights[2] && heights[2] > heights[1]);
        ui.set_table_scroll_offset(table, 0.0, 2000.0);
        ui.set_splitter_ratio(split, 0.6);
        ui.sync(&mut scene);
        assert_eq!(ui.table_scroll_offset(table).unwrap().1, 2000.0);
        let params = FlexParams::row(0.0, 0.0, 100.0, 40.0);
        let measures = ui.flex_child_measures(&params, &[table, toolbar]);
        assert_eq!(measures[0], ChildMeasure::new(0.0, 40.0));
        assert!(ui.set_flex_fill(toolbar, true));
        let measures = ui.flex_child_measures(&params, &[table, toolbar]);
        assert_eq!(measures, vec![ChildMeasure::new(50.0, 40.0); 2]);
        assert!(ui.set_flex_fill(table, false));
    }
}
