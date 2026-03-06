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
    pub reason: String,
    pub created_at: i64,
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
            INSERT INTO switch_events (group_name, from_node, to_node, reason, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                event.group_name,
                event.from_node,
                event.to_node,
                event.reason,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(include_str!("../../migrations/0001_init.sql"))
            .context("初始化 SQLite schema 失败")?;
        Ok(())
    }
}
