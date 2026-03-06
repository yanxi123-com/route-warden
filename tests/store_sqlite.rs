use route_warden::store::{GroupStateRecord, ProbeRecord, SqliteStore, SwitchEventRecord};

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

    let round_id = store.start_round(2_000).unwrap();
    store.finish_round(round_id, 2_030, "ok").unwrap();
    let round = store.load_round(round_id).unwrap().unwrap();
    assert_eq!(round.status, "ok".to_string());
    assert_eq!(round.finished_at, Some(2_030));

    let probe = ProbeRecord {
        round_id,
        group_name: "GLOBAL_BEST".to_string(),
        node_name: "NodeA".to_string(),
        target: "binance".to_string(),
        status_code: Some(403),
        latency_ms: 180.5,
        is_success: true,
        failure_kind: None,
        created_at: 2_001,
    };
    store.save_probe(&probe).unwrap();
    let probes = store.list_probes_by_round(round_id).unwrap();
    assert_eq!(probes.len(), 1);
    assert_eq!(probes[0], probe);
}
