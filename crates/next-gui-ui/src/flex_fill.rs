use super::*;

impl Ui {
    /// Fill the remaining main-axis space in a row/column, sharing it equally
    /// with other fill children. The child also stretches across the parent.
    /// Fixed children retain their measured sizes. Returns false for an unknown ID.
    pub fn set_flex_fill(&mut self, id: WidgetId, fill: bool) -> bool {
        if !self.widgets.contains_key(&id) {
            return false;
        }
        if fill {
            self.flex_fill.insert(id);
        } else {
            self.flex_fill.remove(&id);
        }
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
            .filter(|id| self.flex_fill.contains(id))
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
            .filter(|(id, _)| !self.flex_fill.contains(id))
            .map(|(_, size)| if vertical { size.height } else { size.width })
            .sum();
        let available = (span
            - 2.0 * params.padding
            - params.gap * children.len().saturating_sub(1) as f32
            - fixed)
            .max(0.0);
        for (id, size) in children.iter().zip(&mut measures) {
            if self.flex_fill.contains(id) {
                *size = if vertical {
                    ChildMeasure::new(
                        (params.max_width - 2.0 * params.padding).max(0.0),
                        available / count as f32,
                    )
                } else {
                    ChildMeasure::new(
                        available / count as f32,
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
            assert!(ui.widgets[&table].aux.len() < 150);
            heights.push(t.height);
        }
        assert!(heights[0] > heights[2] && heights[2] > heights[1]);
        let params = FlexParams::row(0.0, 0.0, 100.0, 40.0);
        let measures = ui.flex_child_measures(&params, &[table, toolbar]);
        assert_eq!(measures[0], ChildMeasure::new(0.0, 40.0));
        assert!(ui.set_flex_fill(toolbar, true));
        let measures = ui.flex_child_measures(&params, &[table, toolbar]);
        assert_eq!(measures, vec![ChildMeasure::new(50.0, 40.0); 2]);
        assert!(ui.set_flex_fill(table, false));
    }
}
