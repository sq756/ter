use sqlx::sqlite::SqlitePool;
use serde::{Serialize, Deserialize};
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServerConfig {
    pub id: String,
    pub label: String,
    pub host: String,
    pub user: String,
    pub port: i32,
    pub password_enc: Option<String>,
    pub key_path: Option<String>,
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_url).await?;
        
        // Initial migration
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS server_configs (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                host TEXT NOT NULL,
                user TEXT NOT NULL,
                port INTEGER NOT NULL,
                password_enc TEXT,
                key_path TEXT
            )"
        )
        .execute(&pool)
        .await?;

        // Migration: Terminal Logs
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS terminal_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tab_id TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                content BLOB NOT NULL
            )"
        )
        .execute(&pool)
        .await?;

        // Index for performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tab_id ON terminal_logs(tab_id)")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn append_log(&self, tab_id: &str, content: &[u8]) -> Result<()> {
        sqlx::query("INSERT INTO terminal_logs (tab_id, content) VALUES (?, ?)")
            .bind(tab_id)
            .bind(content)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_logs(&self, tab_id: &str, limit: i32) -> Result<Vec<Vec<u8>>> {
        let rows = sqlx::query_as::<_, (Vec<u8>,)>("SELECT content FROM terminal_logs WHERE tab_id = ? ORDER BY id DESC LIMIT ?")
            .bind(tab_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|r| r.0).rev().collect())
    }

    pub async fn list_servers(&self) -> Result<Vec<ServerConfig>> {
        let servers = sqlx::query_as::<_, ServerConfig>("SELECT * FROM server_configs")
            .fetch_all(&self.pool)
            .await?;
        Ok(servers)
    }

    pub async fn save_server(&self, server: &ServerConfig) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO server_configs (id, label, host, user, port, password_enc, key_path) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&server.id)
        .bind(&server.label)
        .bind(&server.host)
        .bind(&server.user)
        .bind(&server.port)
        .bind(&server.password_enc)
        .bind(&server.key_path)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_server(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM server_configs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
