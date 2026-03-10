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

        Ok(Self { pool })
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
