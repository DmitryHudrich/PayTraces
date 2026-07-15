use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;
use uuid::Uuid;

use domain::error::DomainResult;

use crate::{pg_err, pool_err};

#[derive(Debug, Clone)]
pub struct JobRow {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct JobRepository {
    pool: Pool,
}

impl Clone for JobRepository {
    fn clone(&self) -> Self {
        Self { pool: self.pool.clone() }
    }
}

impl JobRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn create_job(&self, kind: &str, payload: serde_json::Value) -> DomainResult<Uuid> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let row = client
            .query_one(
                "INSERT INTO jobs (kind, status, payload) VALUES ($1, 'pending', $2) RETURNING id",
                &[&kind, &payload],
            )
            .await
            .map_err(pg_err)?;
        Ok(row.get("id"))
    }

    pub async fn set_running(&self, id: Uuid) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client
            .execute(
                "UPDATE jobs SET status = 'running', updated_at = now() WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    pub async fn set_done(&self, id: Uuid) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client
            .execute(
                "UPDATE jobs SET status = 'done', updated_at = now() WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    pub async fn set_failed(&self, id: Uuid, error: &str) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client
            .execute(
                "UPDATE jobs SET status = 'failed', error = $2, updated_at = now() WHERE id = $1",
                &[&id, &error],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    pub async fn get_job(&self, id: Uuid) -> DomainResult<Option<JobRow>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let row = client
            .query_opt(
                "SELECT id, kind, status, payload, error, created_at, updated_at \
                 FROM jobs WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(pg_err)?;
        Ok(row.map(|r| JobRow {
            id: r.get("id"),
            kind: r.get("kind"),
            status: r.get("status"),
            payload: r.get("payload"),
            error: r.get("error"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }
}
