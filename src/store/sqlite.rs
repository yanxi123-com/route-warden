use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupStateRecord {
    pub group_name: String,
    pub current_node: String,
    pub last_switch_ts: Option<i64>,
    pub cooldown_until_ts: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct SwitchEventRecord {
    pub group_name: String,
    pub from_node: String,
    pub to_node: String,
    pub score_gap: f64,
    pub reason: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundRecord {
    pub id: i64,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeRecord {
    pub round_id: i64,
    pub group_name: String,
    pub node_name: String,
    pub target: String,
    pub status_code: Option<i64>,
    pub latency_ms: f64,
    pub is_success: bool,
    pub failure_kind: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeSummaryRecord {
    pub group_name: String,
    pub node_name: String,
    pub total: i64,
    pub success: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub last_probe_at: i64,
}

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("打开 SQLite 数据库失败")?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("打开内存 SQLite 数据库失败")?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn save_group_state(&self, state: &GroupStateRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO group_state (group_name, current_node, last_switch_ts, cooldown_until_ts, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(group_name) DO UPDATE SET
              current_node = excluded.current_node,
              last_switch_ts = excluded.last_switch_ts,
              cooldown_until_ts = excluded.cooldown_until_ts,
              updated_at = excluded.updated_at
            "#,
            params![
                state.group_name,
                state.current_node,
                state.last_switch_ts,
                state.cooldown_until_ts,
                state.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn load_group_state(&self, group_name: &str) -> Result<Option<GroupStateRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT group_name, current_node, last_switch_ts, cooldown_until_ts, updated_at
            FROM group_state
            WHERE group_name = ?1
            "#,
        )?;

        let row = stmt
            .query_row(params![group_name], |row| {
                Ok(GroupStateRecord {
                    group_name: row.get(0)?,
                    current_node: row.get(1)?,
                    last_switch_ts: row.get(2)?,
                    cooldown_until_ts: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .optional()?;

        Ok(row)
    }

    pub fn save_switch_event(&self, event: &SwitchEventRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO switch_events (group_name, from_node, to_node, score_gap, reason, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                event.group_name,
                event.from_node,
                event.to_node,
                event.score_gap,
                event.reason,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_switch_events(&self) -> Result<Vec<SwitchEventRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT group_name, from_node, to_node, score_gap, reason, created_at
            FROM switch_events
            ORDER BY id ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SwitchEventRecord {
                group_name: row.get(0)?,
                from_node: row.get(1)?,
                to_node: row.get(2)?,
                score_gap: row.get(3)?,
                reason: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn start_round(&self, started_at: i64) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO rounds (started_at, status)
            VALUES (?1, 'running')
            "#,
            params![started_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_round(&self, round_id: i64, finished_at: i64, status: &str) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE rounds
            SET finished_at = ?1, status = ?2
            WHERE id = ?3
            "#,
            params![finished_at, status, round_id],
        )?;
        Ok(())
    }

    pub fn save_probe(&self, probe: &ProbeRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO probes (
              round_id, group_name, node_name, target, status_code, latency_ms, is_success, failure_kind, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                probe.round_id,
                probe.group_name,
                probe.node_name,
                probe.target,
                probe.status_code,
                probe.latency_ms,
                probe.is_success,
                probe.failure_kind,
                probe.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn load_round(&self, round_id: i64) -> Result<Option<RoundRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, started_at, finished_at, status
            FROM rounds
            WHERE id = ?1
            "#,
        )?;
        let row = stmt
            .query_row(params![round_id], |row| {
                Ok(RoundRecord {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    finished_at: row.get(2)?,
                    status: row.get(3)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    pub fn list_probes_by_round(&self, round_id: i64) -> Result<Vec<ProbeRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT round_id, group_name, node_name, target, status_code, latency_ms, is_success, failure_kind, created_at
            FROM probes
            WHERE round_id = ?1
            ORDER BY id ASC
            "#,
        )?;

        let rows = stmt.query_map(params![round_id], |row| {
            Ok(ProbeRecord {
                round_id: row.get(0)?,
                group_name: row.get(1)?,
                node_name: row.get(2)?,
                target: row.get(3)?,
                status_code: row.get(4)?,
                latency_ms: row.get(5)?,
                is_success: row.get(6)?,
                failure_kind: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;

        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    pub fn summarize_probes_since(&self, since_ts: i64) -> Result<Vec<ProbeSummaryRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              group_name,
              node_name,
              COUNT(*) AS total,
              SUM(CASE WHEN is_success = 1 THEN 1 ELSE 0 END) AS success,
              AVG(CASE WHEN is_success = 1 THEN latency_ms END) AS avg_latency_ms,
              MAX(created_at) AS last_probe_at
            FROM probes
            WHERE created_at >= ?1
            GROUP BY group_name, node_name
            ORDER BY group_name ASC, success DESC, total DESC
            "#,
        )?;

        let rows = stmt.query_map(params![since_ts], |row| {
            let total: i64 = row.get(2)?;
            let success: i64 = row.get(3)?;
            let rate = if total <= 0 {
                0.0
            } else {
                success as f64 / total as f64
            };
            Ok(ProbeSummaryRecord {
                group_name: row.get(0)?,
                node_name: row.get(1)?,
                total,
                success,
                success_rate: rate,
                avg_latency_ms: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                last_probe_at: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for item in rows {
            out.push(item?);
        }
        Ok(out)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(include_str!("../../migrations/0001_init.sql"))
            .context("初始化 SQLite schema 失败")?;
        match self.conn.execute_batch(include_str!(
            "../../migrations/0002_switch_events_score_gap.sql"
        )) {
            Ok(_) => {}
            Err(err) if is_duplicate_column_error(&err) => {}
            Err(err) => return Err(err).context("执行 SQLite 迁移 0002 失败"),
        }
        Ok(())
    }
}

fn is_duplicate_column_error(err: &rusqlite::Error) -> bool {
    let rusqlite::Error::SqliteFailure(_, Some(msg)) = err else {
        return false;
    };
    msg.contains("duplicate column name")
}
