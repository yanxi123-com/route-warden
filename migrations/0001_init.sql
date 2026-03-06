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
