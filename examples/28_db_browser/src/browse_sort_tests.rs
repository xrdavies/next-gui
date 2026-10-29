use super::*;
use crate::{db::OrderBy, project::ProjectFile, state::BrowseTabState};

fn demo() -> Demo {
    let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch(
            "CREATE TABLE t(\" group \" TEXT, \"score ▲\" INTEGER, id INTEGER);
         INSERT INTO t VALUES ('a',10,1),('a',30,2),('b',20,3),('a',20,4),('b',30,5);",
        )
        .unwrap();
    demo.state.refresh_schema().unwrap();
    demo.state.browse_mut().table = Some("t".into());
    demo.state.refresh_browse().unwrap();
    demo.state.write_changes().unwrap();
    demo.state.tab = MainTab::Browse;
    demo.do_rebuild();
    demo
}

fn click_header(demo: &mut Demo, column: usize, append: bool) {
    let bounds = demo
        .scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.browse_table))
        .unwrap()
        .bounds;
    let widths = demo.ui.table_column_widths(demo.ids.browse_table).unwrap();
    let x = bounds.x + widths[..column].iter().sum::<f32>() + 20.0;
    assert!(demo.ui.pointer_pressed(&mut demo.scene, x, bounds.y + 10.0));
    demo.snapshot_inputs();
    demo.poll_header_sort(append);
    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
    demo.do_rebuild();
}

fn ids(demo: &Demo) -> Vec<&str> {
    demo.state
        .browse()
        .result
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|r| r[2].as_str())
        .collect()
}

#[test]
fn multi_sort_keeps_priority_toggles_columns_and_clears_explicitly() {
    let mut demo = demo();
    click_header(&mut demo, 0, false);
    click_header(&mut demo, 1, true);
    assert_eq!(ids(&demo), ["1", "4", "2", "3", "5"]);
    click_header(&mut demo, 1, true);
    assert_eq!(ids(&demo), ["2", "4", "1", "5", "3"]);
    click_header(&mut demo, 2, true);
    let ordering = demo.state.browse().ordering();
    assert_eq!(
        ordering
            .iter()
            .map(|o| (o.column.as_str(), o.descending))
            .collect::<Vec<_>>(),
        [(" group ", false), ("score ▲", true), ("id", false)]
    );
    assert_eq!(demo.state.browse().sel_row, Some(0));
    assert_eq!(demo.state.browse().sel_col, Some(2));
    assert_eq!(demo.state.edit_value, "2");
    assert_eq!(
        demo.ui.button_enabled(demo.ids.btn_browse_clear_sort),
        Some(true)
    );
    if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
        demo.scene.mark_full_dirty(1400.0, 860.0);
        let dirty = demo.scene.take_dirty();
        let mut renderer = SoftRenderer::new(1400, 860, clear_color(&demo.theme));
        renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
        write_ppm(&path, renderer.frame_pixels().unwrap().0, 1400, 860).unwrap();
    }
    // A normal click replaces the list, and repeated normal clicks alternate.
    click_header(&mut demo, 1, false);
    assert_eq!(demo.state.browse().ordering().len(), 1);
    assert_eq!(demo.state.browse().order_col.as_deref(), Some("score ▲"));
    assert!(!demo.state.browse().order_desc);
    click_header(&mut demo, 1, false);
    assert!(demo.state.browse().order_desc);
    click_header(&mut demo, 1, false);
    assert!(!demo.state.browse().order_desc);
    demo.handle_button(demo.ids.btn_browse_clear_sort);
    demo.do_rebuild();
    assert!(demo.state.browse().ordering().is_empty());
    assert_eq!(
        demo.ui.button_enabled(demo.ids.btn_browse_clear_sort),
        Some(false)
    );
    assert_eq!(ids(&demo), ["1", "2", "3", "4", "5"]);
}

#[test]
fn sorting_guards_drafts_and_restores_tab_on_query_error() {
    let mut demo = demo();
    click_header(&mut demo, 1, false);
    demo.ui.set_text_area_value(demo.ids.edit_value, "99");
    demo.snapshot_inputs();
    let previous = demo.state.browse().clone();
    assert!(demo
        .state
        .sort_browse_column("id".into(), true)
        .unwrap_err()
        .contains("draft"));
    assert!(demo
        .state
        .set_browse_ordering(Vec::new())
        .unwrap_err()
        .contains("draft"));
    assert_eq!(demo.state.edit_value, "99");
    assert_eq!(demo.state.browse().ordering(), previous.ordering());
    demo.state.discard_cell_draft();
    demo.state.browse_mut().offset = 500;
    demo.state.browse_mut().filter = "missing_function()".into();
    let previous = demo.state.browse().clone();
    assert!(demo.state.sort_browse_column("id".into(), true).is_err());
    assert_eq!(demo.state.browse().ordering(), previous.ordering());
    assert_eq!(demo.state.browse().offset, 500);
    assert_eq!(demo.state.browse().row_keys, previous.row_keys);
    assert_eq!(demo.state.browse().sel_row, previous.sel_row);
    assert_eq!(demo.state.browse().sel_col, previous.sel_col);
    assert_eq!(
        demo.state.browse().result.as_ref().unwrap().rows,
        previous.result.unwrap().rows
    );
    assert!(!demo.state.dirty);
}

#[test]
fn project_multi_sort_roundtrip_keeps_native_priority_and_legacy_primary() {
    let demo = demo();
    let conn = demo.state.conn.as_ref().unwrap();
    let source = r#"<sqlb_project><db path="data.db"/><tab_browse><table table="4,1:maint"/>
        <browse_table_settings><table schema="main" name="t"><sort>
        <column index="0" mode="0"/><column index="1" mode="1"/>
        <column index="2" mode="0"/></sort></table></browse_table_settings></tab_browse></sqlb_project>"#;
    let mut project = crate::project_xml::decode(source).unwrap();
    crate::project_xml::restore_browse(&mut project, conn).unwrap();
    let expected = project.browse_tabs[0].ordering();
    assert_eq!(expected.len(), 3);
    assert!(expected[1].descending);
    crate::project_xml::capture_connection(&mut project, conn).unwrap();
    let encoded =
        crate::project_xml::encode(&project, std::path::Path::new("test.sqbpro")).unwrap();
    let mut restored = crate::project_xml::decode(&encoded).unwrap();
    crate::project_xml::restore_browse(&mut restored, conn).unwrap();
    assert_eq!(restored.browse_tabs[0].ordering(), expected);
    assert!(!crate::project_xml::unapplied_settings(&restored)
        .unwrap()
        .iter()
        .any(|s| s.contains("advanced browse")));
    restored.browse_tabs[0].set_ordering(Vec::new());
    crate::project_xml::capture_connection(&mut restored, conn).unwrap();
    let encoded =
        crate::project_xml::encode(&restored, std::path::Path::new("test.sqbpro")).unwrap();
    let mut cleared = crate::project_xml::decode(&encoded).unwrap();
    crate::project_xml::restore_browse(&mut cleared, conn).unwrap();
    assert!(cleared.browse_tabs[0].ordering().is_empty());

    let legacy: BrowseTabState = serde_json::from_str(r#"{"title":"t","table":"t","filter":"","col_filters":[],"order_col":"score ▲","order_desc":true}"#).unwrap();
    assert_eq!(
        legacy.ordering(),
        [OrderBy {
            column: "score ▲".into(),
            descending: true
        }]
    );
    let json = serde_json::to_string(&project).unwrap();
    let from_json: ProjectFile = serde_json::from_str(&json).unwrap();
    assert_eq!(from_json.browse_tabs[0].ordering(), expected);
}

#[test]
fn complete_exports_follow_multi_sort_and_keep_the_destination_on_error() {
    let mut demo = demo();
    demo.state
        .sort_browse_column(" group ".into(), false)
        .unwrap();
    demo.state
        .sort_browse_column("score ▲".into(), true)
        .unwrap();
    demo.state
        .sort_browse_column("score ▲".into(), true)
        .unwrap();
    demo.state.browse_mut().offset = 500;
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("sorted.csv");
    let json = dir.path().join("sorted.json");
    demo.state.export_browse(&csv, false).unwrap();
    demo.state.export_browse(&json, true).unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [2, 4, 1, 5, 3]
    );
    let text = std::fs::read_to_string(&csv).unwrap();
    assert_eq!(
        text.lines()
            .skip(1)
            .map(|line| line.rsplit(',').next().unwrap())
            .collect::<Vec<_>>(),
        ["2", "4", "1", "5", "3"]
    );
    demo.state.browse_mut().additional_order.push(OrderBy {
        column: "missing".into(),
        descending: false,
    });
    assert!(demo.state.export_browse(&csv, false).is_err());
    assert_eq!(std::fs::read_to_string(&csv).unwrap(), text);
}

#[test]
fn query_sort_quotes_literal_identifiers_and_rejects_missing_columns() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE t(\" x \" INTEGER, \"q\"\"; DROP TABLE t;--\" INTEGER);
        INSERT INTO t VALUES(2,1),(1,2),(1,1)",
    )
    .unwrap();
    let ordering = [
        OrderBy {
            column: " x ".into(),
            descending: false,
        },
        OrderBy {
            column: "q\"; DROP TABLE t;--".into(),
            descending: true,
        },
    ];
    for rowid in [false, true] {
        let query = |conn: &rusqlite::Connection, table: &str, order: &[OrderBy], filter: Option<&str>, limit, offset| {
            if rowid { dbmod::browse_with_keys_in(conn,"main",table,order,filter,limit,offset).map(|(result,_)|result) }
            else { dbmod::browse_table(conn,table,order,filter,limit,offset) }
        };
        let result = query(&conn, "t", &ordering, None, 2, 1).unwrap();
        let column = 0;
        assert_eq!(
            result
                .rows
                .iter()
                .map(|r| (r[column].as_str(), r[column + 1].as_str()))
                .collect::<Vec<_>>(),
            [("1", "1"), ("2", "1")]
        );
        assert!(query(
            &conn,
            "t",
            &[OrderBy {
                column: "missing".into(),
                descending: false
            }],
            None,
            10,
            0
        )
        .is_err());
    }
    assert_eq!(dbmod::count_rows(&conn, "t").unwrap(), 3);
}
