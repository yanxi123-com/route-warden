use route_warden::store::{GroupStateRecord, ProbeRecord, SqliteStore, SwitchEventRecord};
use tempfile::tempdir;

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
        score_gap: 0.123,
        reason: "stable_better_candidate".to_string(),
        created_at: 1_201,
    };
    store.save_switch_event(&event).unwrap();
    let events = store.list_switch_events().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].score_gap, 0.123);

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

    let summary = store.summarize_probes_since(0).unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].group_name, "GLOBAL_BEST".to_string());
    assert_eq!(summary[0].node_name, "NodeA".to_string());
    assert_eq!(summary[0].total, 1);
    assert_eq!(summary[0].success, 1);
}

#[test]
fn migrate_old_db_adds_switch_event_score_gap() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("legacy.sqlite3");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS rounds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS probes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  round_id INTEGER NOT NULL,
  group_name TEXT NOT NULL,
  node_name TEXT NOT NULL,
  target TEXT NOT NULL,
  status_code INTEGER,
  latency_ms REAL,
  is_success INTEGER NOT NULL,
  failure_kind TEXT,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(round_id) REFERENCES rounds(id)
);

CREATE TABLE IF NOT EXISTS switch_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  group_name TEXT NOT NULL,
  from_node TEXT NOT NULL,
  to_node TEXT NOT NULL,
  reason TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS group_state (
  group_name TEXT PRIMARY KEY,
  current_node TEXT NOT NULL,
  last_switch_ts INTEGER,
  cooldown_until_ts INTEGER,
  updated_at INTEGER NOT NULL
);
"#,
        )
        .unwrap();
    }

    let store = SqliteStore::open(&db_path).unwrap();
    let event = SwitchEventRecord {
        group_name: "GLOBAL_BEST".to_string(),
        from_node: "NodeA".to_string(),
        to_node: "NodeB".to_string(),
        score_gap: 0.3,
        reason: "stable_better_candidate".to_string(),
        created_at: 10,
    };
    store.save_switch_event(&event).unwrap();
    let events = store.list_switch_events().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].score_gap, 0.3);
}
