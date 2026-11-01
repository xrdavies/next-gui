//! Cached structure rows share the qualified catalog used by SQL completion.
use crate::{db::{self,ObjectKind},sql_completion,state::AppState};
use next_gui_ui::{TreeTableNode,Ui,WidgetId};

const FIELD:u64=1<<63;
#[derive(Debug,Clone)]
pub struct Field { pub name:String,pub kind:String,pub definition:String }
pub fn load(schema:&[db::SchemaObject],catalog:&sql_completion::Catalog)->Vec<Vec<Field>> {
    schema.iter().map(|object| {
        if object.kind==ObjectKind::Index {
            return object.sql.as_deref().and_then(|sql|crate::table_sql::structure_index_fields(sql).ok()).unwrap_or_default().into_iter().map(|(name,kind,definition)|Field{name,kind,definition}).collect();
        }
        let definitions=object.sql.as_deref().and_then(|sql|crate::table_sql::field_definitions(sql).ok()).unwrap_or_default();
        catalog.column_types_in(&object.schema,&object.name).iter().map(|(name,kind)| Field {
            name:name.clone(),kind:kind.clone(),definition:definitions.iter().find(|(column,_)|column==name).map(|(_,sql)|sql.clone()).unwrap_or_else(||format!("{} {}",db::quote_ident(name),kind))
        }).collect()
    }).collect()
}
pub fn field_id(object:usize,column:usize)->u64 { FIELD|((object as u64)<<32)|column as u64 }
pub fn field_at(state:&AppState,node:u64)->Option<(usize,&Field)> {
    if node&FIELD==0 {return None;}
    let object=((node&!FIELD)>>32) as usize;let column=(node&u32::MAX as u64) as usize;
    Some((object,state.structure_fields.get(object)?.get(column)?))
}
pub fn object_index(state:&AppState,node:u64)->Option<usize> {
    if node&FIELD!=0 {field_at(state,node).map(|(object,_)|object)}
    else {node.checked_sub(10000).and_then(|index|state.schema.get(index as usize).map(|_|index as usize))}
}
pub fn selected_node(state:&AppState,index:usize)->u64 {
    state.schema_field.as_ref().and_then(|name|state.structure_fields.get(index)?.iter().position(|field|field.name==*name)).map_or(10000+index as u64,|column|field_id(index,column))
}
pub fn model(state:&AppState)->Vec<TreeTableNode> {
    state.schema_names.iter().enumerate().map(|(database,name)| {
        let root=database as u64*5+1;
        let groups=[ObjectKind::Table,ObjectKind::View,ObjectKind::Index,ObjectKind::Trigger].into_iter().enumerate().map(|(group,kind)| {
            let objects=state.schema.iter().enumerate().filter(|(_,object)|object.schema==*name&&object.kind==kind).map(|(index,object)| {
                let fields=state.structure_fields.get(index).into_iter().flatten().enumerate().map(|(column,field)|TreeTableNode::leaf(field_id(index,column),&field.name,vec![field.kind.clone(),field.definition.clone()])).collect();
                let mut node=TreeTableNode::new(10000+index as u64,&object.name,vec![String::new(),object.sql.clone().unwrap_or_default()],fields);
                node.expanded=state.structure_expanded.get(&(object.schema.clone(),object.kind.sql_type().into(),object.name.clone())).copied().unwrap_or_else(||state.selected.as_ref().is_some_and(|selected|selected.schema==object.schema&&selected.name==object.name&&selected.kind==object.kind));node
            }).collect::<Vec<_>>();
            TreeTableNode::new(root+group as u64+1,format!("{} ({})",kind.label(),objects.len()),Vec::new(),objects)
        }).collect();
        TreeTableNode::new(root,name,Vec::new(),groups)
    }).collect()
}
pub fn identities(state:&AppState)->Vec<(String,String,String)> {
    state.schema.iter().map(|object|(object.schema.clone(),object.kind.sql_type().into(),object.name.clone())).collect()
}
pub fn snapshot(state:&mut AppState,ui:&Ui,tree:WidgetId,objects:&[(String,String,String)]) {
    if let Some(mut widths)=ui.table_column_widths(tree) {widths.truncate(2);state.structure_widths=widths;}
    for (index,identity) in objects.iter().enumerate() {
        if let Some(expanded)=ui.tree_expanded(tree,10000+index as u64) { state.structure_expanded.insert(identity.clone(),expanded); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{browse_filter::tests::fixture,MainTab,UiKey};
    use next_gui_ui::Theme;

    #[test]
    fn structure_fields_are_qualified_and_preserve_types_constraints_and_index_expressions() {
        let mut state=AppState::empty();state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch(r#"
            CREATE TABLE records(id INTEGER PRIMARY KEY, name TEXT NOT NULL DEFAULT 'kept' COLLATE NOCASE CHECK(length(name)>0),parent INTEGER REFERENCES records(id),total REAL GENERATED ALWAYS AS(id*1.2) STORED);
            CREATE INDEX named ON records(name COLLATE NOCASE DESC,lower(name),id ASC);
            CREATE VIEW visible AS SELECT id,name FROM records;
            CREATE TEMP TABLE records(temporary BLOB);
            ATTACH ':memory:' AS "aux.""db";
            CREATE TABLE "aux.""db".records("quoted field" NUMERIC DEFAULT 7);
        "#).unwrap();state.refresh_schema().unwrap();
        let fields=|schema:&str,name:&str| &state.structure_fields[state.schema.iter().position(|o|o.schema==schema&&o.name==name).unwrap()];
        let main=fields("main","records");assert_eq!(main.len(),4);assert_eq!(main[1].kind,"TEXT");
        for fragment in ["NOT NULL","DEFAULT 'kept'","COLLATE NOCASE","CHECK"] {assert!(main[1].definition.contains(fragment),"{}",main[1].definition);}
        assert!(main[0].definition.contains("PRIMARY KEY"));assert!(main[2].definition.contains("REFERENCES"));assert!(main[3].definition.contains("STORED"));
        assert_eq!(fields("temp","records")[0].kind,"BLOB");assert_eq!(fields("aux.\"db","records")[0].name,"quoted field");
        assert_eq!(fields("main","visible")[1].kind,"TEXT");
        let index=fields("main","named");assert_eq!(index.len(),3);assert_eq!(index[0].kind,"DESC");assert!(index[0].definition.contains("COLLATE NOCASE"));assert!(index[1].name.contains("lower"));assert_eq!(index[2].kind,"ASC");
        let rows=model(&state);assert_eq!(rows.len(),3);
        assert!(!state.dirty);assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM main.records",[],|r|r.get::<_,usize>(0)).unwrap(),0);
    }

    #[test]
    fn desktop_structure_field_navigation_preserves_ddl_and_qualified_expansion() {
        let mut demo=fixture(1600,900,Theme::dark());demo.state.write_changes().unwrap();demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,name TEXT NOT NULL);CREATE TABLE second(value);ATTACH ':memory:' AS aux;CREATE TABLE aux.records(other BLOB);").unwrap();demo.state.refresh_schema().unwrap();demo.state.tab=MainTab::Structure;
        let main=demo.state.schema.iter().position(|o|o.schema=="main"&&o.name=="records").unwrap();let aux=demo.state.schema.iter().position(|o|o.schema=="aux"&&o.name=="records").unwrap();
        demo.state.select_schema_node(10000+main as u64);demo.do_rebuild();demo.ui.set_focus(Some(demo.ids.tree));
        demo.ui.set_tree_expanded(demo.ids.tree,10000+main as u64,true);demo.ui.key_input(UiKey::Right);demo.poll_selection_changes();demo.do_rebuild();
        assert_eq!(demo.state.schema_field.as_deref(),Some("id"));assert_eq!(demo.ui.tree_selected(demo.ids.tree),Some(field_id(main,0)));assert_eq!(demo.ui.focused(),Some(demo.ids.tree));
        demo.ui.set_table_column_width(demo.ids.tree,0,340.0);demo.ui.set_table_column_width(demo.ids.tree,1,160.0);demo.snapshot_inputs();demo.do_rebuild();assert_eq!(&demo.ui.table_column_widths(demo.ids.tree).unwrap()[..2],&[340.0,160.0]);
        let draft=format!("{} -- retained draft",demo.state.ddl_draft);demo.ui.set_text_area_value(demo.ids.ddl_area,&draft);demo.snapshot_inputs();
        demo.ui.key_input(UiKey::Down);demo.poll_selection_changes();demo.do_rebuild();assert_eq!(demo.state.schema_field.as_deref(),Some("name"));assert_eq!(demo.state.ddl_draft,draft);assert!(!demo.state.confirm_open);
        demo.ui.set_tree_expanded(demo.ids.tree,10000+aux as u64,false);demo.snapshot_inputs();let identities=identities(&demo.state);
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE aaa(new_column);").unwrap();demo.state.refresh_schema().unwrap();snapshot(&mut demo.state,&demo.ui,demo.ids.tree,&identities);demo.do_rebuild();
        let main=demo.state.schema.iter().position(|o|o.schema=="main"&&o.name=="records").unwrap();let aux=demo.state.schema.iter().position(|o|o.schema=="aux"&&o.name=="records").unwrap();
        assert_eq!(demo.ui.tree_selected(demo.ids.tree),Some(field_id(main,1)));assert_eq!(demo.ui.tree_expanded(demo.ids.tree,10000+main as u64),Some(true));assert_eq!(demo.ui.tree_expanded(demo.ids.tree,10000+aux as u64),Some(false));
        demo.ui.set_tree_selected(demo.ids.tree,Some(field_id(aux,0)));demo.poll_selection_changes();assert!(demo.state.confirm_open);assert_eq!(demo.state.selected.as_ref().unwrap().schema,"main");assert_eq!(demo.state.ddl_draft,draft);
        let old:crate::project::ProjectFile=serde_json::from_str(r#"{"db_path":":memory:","structure_ratio":0.34}"#).unwrap();assert_eq!(old.structure_ratio,0.68);
        let mut project=crate::project::ProjectFile::from_state(&demo.state);project.structure_ratio=0.61;let encoded=serde_json::to_string(&project).unwrap();let restored:crate::project::ProjectFile=serde_json::from_str(&encoded).unwrap();assert_eq!(restored.structure_ratio,0.61);assert_eq!(restored.structure_widths,vec![340.0,160.0]);assert!(encoded.contains("structure_rows_ratio"));
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("desktop.sqbpro");project.save(&path).unwrap();let reopened=crate::project::ProjectFile::load(&path).unwrap();let mut state=AppState::empty();reopened.apply_to(&mut state);assert_eq!(state.structure_ratio,0.61);assert_eq!(state.structure_widths,vec![340.0,160.0]);
    }
}
