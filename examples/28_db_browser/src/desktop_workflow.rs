//! Reproducible desktop scenario; all data is synthetic and stays in memory.
use crate::state::{AppState, MainTab};

const SCHEMA: &str = "
CREATE TABLE customers(id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT, country TEXT);
CREATE TABLE products(id INTEGER PRIMARY KEY, sku TEXT UNIQUE NOT NULL, name TEXT NOT NULL, unit_price REAL NOT NULL);
CREATE TABLE orders(
 id INTEGER PRIMARY KEY, customer_id INTEGER NOT NULL REFERENCES customers(id),
 created_at TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending','paid','shipped','cancelled')),
 currency TEXT NOT NULL DEFAULT 'USD', subtotal REAL NOT NULL, tax REAL NOT NULL, shipping REAL NOT NULL,
 total REAL GENERATED ALWAYS AS (round(subtotal+tax+shipping,2)) STORED, note TEXT, updated_at TEXT);
CREATE TABLE order_items(id INTEGER PRIMARY KEY, order_id INTEGER NOT NULL REFERENCES orders(id), product_id INTEGER NOT NULL REFERENCES products(id), quantity INTEGER NOT NULL, unit_price REAL NOT NULL);
CREATE INDEX orders_customer ON orders(customer_id);
CREATE INDEX orders_status_date ON orders(status,created_at);
CREATE INDEX items_order ON order_items(order_id);
CREATE VIEW order_summary AS SELECT o.id,c.name,o.created_at,o.status,o.total FROM orders o JOIN customers c ON c.id=o.customer_id;
CREATE VIEW monthly_sales AS SELECT substr(created_at,1,7) AS month,count(*) AS orders,round(sum(total),2) AS revenue FROM orders GROUP BY month;
CREATE TRIGGER order_timestamp AFTER UPDATE OF note ON orders BEGIN UPDATE orders SET updated_at='2026-10-31 12:00:00' WHERE id=NEW.id; END;
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<500) INSERT INTO customers SELECT x,printf('Customer %03d',x),printf('customer%03d@example.test',x),CASE x%3 WHEN 0 THEN 'Japan' WHEN 1 THEN 'Germany' ELSE 'Canada' END FROM n;
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000) INSERT INTO products SELECT x,printf('SKU-%05d',x),printf('Product %04d',x),round(5.0+x*0.37,2) FROM n;
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100000) INSERT INTO orders(id,customer_id,created_at,status,currency,subtotal,tax,shipping,note,updated_at)
SELECT x,1+x%500,printf('2026-%02d-%02d',1+x%10,1+x%28),CASE x%4 WHEN 0 THEN 'pending' WHEN 1 THEN 'paid' WHEN 2 THEN 'shipped' ELSE 'cancelled' END,'USD',round(20+x%900*1.23,2),round((20+x%900*1.23)*0.08,2),4.95,CASE WHEN x%5=0 THEN 'Deliver before 5 pm' ELSE 'Standard delivery' END,'2026-10-31 09:30:00' FROM n;
WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<200000) INSERT INTO order_items SELECT x,1+(x-1)/2,1+x%1000,1+x%4,round(5.0+(x%1000)*0.37,2) FROM n;
";
const REPORT: &str = "-- Revenue and order volume by status\nSELECT status,\n       COUNT(*) AS order_count,\n       ROUND(SUM(total), 2) AS revenue,\n       ROUND(AVG(total), 2) AS average_order\nFROM orders\nGROUP BY status\nORDER BY revenue DESC;";

pub const DELIVERY_UPDATE: &str = "-- Revise delivery for the final four orders\nWITH deliveries(id) AS (\n    SELECT id FROM orders WHERE id > 99996\n)\nUPDATE orders\nSET shipping = shipping + 1, note = 'Priority delivery revised'\nWHERE id IN deliveries\nRETURNING id, shipping, note;";

pub fn prepare(state: &mut AppState, page: &str) -> Result<(), String> {
    state.new_memory_db()?;
    state.conn.as_ref().unwrap().execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    state.refresh_schema()?;
    state.write_changes()?;
    if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_DESKTOP_DB") { state.save_as(&path)?; }
    let index = state.schema.iter().position(|object| object.schema == "main" && object.name == "orders").unwrap();
    state.select_schema_node(10_000 + index as u64);
    state.browse_mut().sel_row = Some(0); state.browse_mut().sel_col = Some(9);
    state.sync_edit_cell_from_selection();
    state.tab = if page.starts_with("desktop-browse") { MainTab::Browse }
        else if page.starts_with("desktop-pragmas") { MainTab::Pragmas }
        else if page.starts_with("desktop-sql") { MainTab::ExecuteSql } else { MainTab::Structure };
    state.sql_tabs[0] = if page.starts_with("desktop-sql-messages") { DELIVERY_UPDATE }
        else if page.starts_with("desktop-sql-error") { "-- Inspect a missing order column\nSELECT id, delivery_reference FROM orders;" }
        else { REPORT }.into();
    state.mark_sql_saved();
    if state.tab == MainTab::ExecuteSql { state.start_sql_job()?; }
    state.checkpoint_project();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{browse_filter::tests::{capture, fixture}, UiKey};
    use next_gui_ui::Theme;
    use std::time::{Duration,Instant};

    fn finish_sql(state: &mut AppState) {
        let end=Instant::now()+Duration::from_secs(15);
        while state.sql_busy { assert!(Instant::now()<end);state.poll_sql_job();std::thread::sleep(Duration::from_millis(1)); }
        assert!(state.error_msg.is_none(),"{:?}",state.error_msg);
    }

    #[test]
    fn desktop_primary_database_workflow_filters_edits_reverts_queries_and_reopens() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("orders.sqlite");
        let mut state=AppState::empty();prepare(&mut state,"desktop-browse").unwrap();state.save_as(path.to_str().unwrap()).unwrap();
        assert_eq!(state.browse().total_rows,Some(100000));
        state.browse_mut().filter="status='paid'".into();state.refresh_browse().unwrap();
        assert_eq!(state.browse().total_rows,Some(25000));
        state.browse_mut().filter.clear();state.refresh_browse().unwrap();state.goto_browse_record(100000).unwrap();
        let row=state.browse().sel_row.unwrap();assert_eq!(state.browse().offset+row+1,100000);
        state.browse_mut().sel_col=Some(9);state.sync_edit_cell_from_selection();
        let original=state.edit_value.clone();
        state.begin_browse_cell_edit(row,9).unwrap();state.edit_value="priority pending".into();state.apply_cell_draft().unwrap();
        assert!(state.dirty);
        let reader=crate::db::open_readonly(path.to_str().unwrap()).unwrap();
        assert_eq!(reader.query_row("SELECT note FROM orders WHERE id=100000",[],|row|row.get::<_,String>(0)).unwrap(),original);
        state.revert_changes().unwrap();state.goto_browse_record(100000).unwrap();
        let row=state.browse().sel_row.unwrap();state.begin_browse_cell_edit(row,9).unwrap();state.edit_value="priority saved".into();state.apply_cell_draft().unwrap();state.write_changes().unwrap();
        assert_eq!(reader.query_row("SELECT note FROM orders WHERE id=100000",[],|row|row.get::<_,String>(0)).unwrap(),"priority saved");
        state.start_sql_text(REPORT.into()).unwrap();finish_sql(&mut state);
        let count:usize=state.sql_result.as_ref().unwrap().rows.iter().map(|row|row[1].parse::<usize>().unwrap()).sum();assert_eq!(count,100000);
        state.start_sql_text("ALTER TABLE orders ADD COLUMN tracking_code TEXT".into()).unwrap();finish_sql(&mut state);state.write_changes().unwrap();
        let mut reopened=AppState::empty();reopened.open_path(path.to_str().unwrap()).unwrap();reopened.switch_browse_table_in("main","orders").unwrap();
        assert_eq!(reopened.browse().total_rows,Some(100000));assert!(reopened.browse().result.as_ref().unwrap().columns.contains(&"tracking_code".into()));
        assert_eq!(reopened.conn.as_ref().unwrap().query_row("SELECT count(*) FROM order_items",[],|row|row.get::<_,usize>(0)).unwrap(),200000);
        assert_eq!(reopened.conn.as_ref().unwrap().query_row("SELECT note FROM orders WHERE id=100000",[],|row|row.get::<_,String>(0)).unwrap(),"priority saved");
    }

    #[test]
    fn desktop_primary_pages_use_the_workspace_and_keep_manual_column_widths() {
        for (name,theme,width,height) in [("dark",Theme::dark(),1600,900),("light",Theme::light(),1920,927)] {
            let mut demo=fixture(width,height,theme);demo.state.write_changes().unwrap();prepare(&mut demo.state,"desktop-structure").unwrap();demo.do_rebuild();
            let bounds=|demo:&crate::Demo,id| demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(id)).unwrap().bounds;
            for (page,tab) in [("structure",MainTab::Structure),("browse",MainTab::Browse),("pragmas",MainTab::Pragmas),("sql",MainTab::ExecuteSql)] {
                demo.ui.set_tabs_selected(demo.ids.tabs,tab.index());demo.poll_selection_changes();demo.do_rebuild();assert_eq!(demo.state.tab,tab);
                assert!(bounds(&demo,demo.ids.tabs).width>width as f32*0.68);
                if tab==MainTab::Browse {
                    let grid=bounds(&demo,demo.ids.browse_table);
                    assert!(grid.height>=height as f32*0.58,"the primary grid should use the available desktop: {grid:?}");
                    let viewport=bounds(&demo,demo.ids.page_scroll);
                    let navigation=bounds(&demo,demo.ids.btn_record_last);
                    assert!(navigation.y>=grid.y+grid.height && viewport.y+viewport.height-navigation.y-navigation.height<32.0,
                        "record navigation should sit at the bottom of the primary workspace: {grid:?}, {navigation:?}, {viewport:?}");
                    assert_eq!(demo.ui.scroll_offset(demo.ids.page_scroll),Some((0.0,0.0)));
                    let baseline=bounds(&demo,demo.ids.browse_select);
                    for id in [demo.ids.browse_select,demo.ids.btn_add_row,demo.ids.btn_del_row,demo.ids.btn_form_view,demo.ids.btn_freeze,demo.ids.btn_cond_fmt,demo.ids.btn_fk_nav,demo.ids.browse_global_filter,demo.ids.browse_filter,demo.ids.btn_browse_go,demo.ids.btn_browse_clear_filters,demo.ids.btn_browse_clear_sort] {
                        let rect=bounds(&demo,id);
                        assert!((rect.y+rect.height*0.5-baseline.y-baseline.height*0.5).abs()<1.0,"record/filter actions should share one row: {rect:?}");
                        assert!(rect.x>=0.0 && rect.x+rect.width<=grid.x+grid.width,"primary action is outside the desktop viewport: {rect:?}, {grid:?}");
                        if demo.ui.button_enabled(id)==Some(true) || [demo.ids.browse_select,demo.ids.browse_global_filter,demo.ids.browse_filter].contains(&id) {
                            assert_eq!(demo.ui.hit_widget(&demo.scene,rect.x+rect.width*0.5,rect.y+rect.height*0.5),Some(id));
                        }
                    }
                    let widths=demo.ui.table_column_widths(demo.ids.browse_table).unwrap();assert!(widths[2]>100.0);assert!(widths[9]>120.0);assert!(widths[0]<widths[9]);
                    demo.ui.set_table_column_width(demo.ids.browse_table,9,245.0);demo.snapshot_inputs();demo.do_rebuild();assert_eq!(demo.ui.table_column_widths(demo.ids.browse_table).unwrap()[9],245.0);
                    demo.state.browse_view=crate::state::BrowseViewMode::Form;demo.do_rebuild();
                    let navigation=bounds(&demo,demo.ids.btn_record_last);
                    let viewport=bounds(&demo,demo.ids.page_scroll);
                    assert!(viewport.y+viewport.height-navigation.y-navigation.height<32.0);
                    assert_eq!(demo.ui.scroll_offset(demo.ids.page_scroll),Some((0.0,0.0)));
                    let first=bounds(&demo,demo.ids.form_fields[0]);
                    assert!(first.y>viewport.y && first.y+first.height<navigation.y);
                    capture(&mut demo,&format!("desktop-form-{name}-{width}"));
                    demo.state.browse_view=crate::state::BrowseViewMode::Table;demo.do_rebuild();
                }
                if tab==MainTab::ExecuteSql {
                    demo.state.start_sql_job().unwrap();finish_sql(&mut demo.state);demo.do_rebuild();
                    assert!(bounds(&demo,demo.ids.sql_area).height>250.0);assert!(bounds(&demo,demo.ids.sql_table).height>=96.0);
                    assert!(bounds(&demo,demo.ids.sql_messages.area).height>=64.0);
                    assert_eq!(demo.state.sql_result.as_ref().unwrap().rows.len(),4);
                }
                capture(&mut demo,&format!("desktop-{page}-{name}-{width}"));
            }
            demo.state.tab=MainTab::Browse;demo.do_rebuild();demo.ui.set_focus(Some(demo.ids.browse_table));demo.ui.key_input(UiKey::Home);
            assert_eq!(demo.ui.table_column_widths(demo.ids.browse_table).unwrap()[9],245.0);
        }
    }
}
