use route_warden::store::{GroupStateRecord, SqliteStore, SwitchEventRecord};

#[test]
fn persist_and_restore_group_state() {
    let store = SqliteStore::open_in_memory().unwrap();

    let state = GroupStateRecord {
        group_name: "GLOBAL_BEST".to_string(),
        current_node: "NodeA".to_string(),
        last_switch_ts: Some(1_000),
        cooldown_until_ts: Some(1_600),
        updated_at: 1_200,
    };

    store.save_group_state(&state).unwrap();
    let loaded = store.load_group_state("GLOBAL_BEST").unwrap().unwrap();
    assert_eq!(loaded, state);

    let event = SwitchEventRecord {
        group_name: "GLOBAL_BEST".to_string(),
        from_node: "NodeA".to_string(),
        to_node: "NodeB".to_string(),
        reason: "stable_better_candidate".to_string(),
        created_at: 1_201,
    };
    store.save_switch_event(&event).unwrap();
}
