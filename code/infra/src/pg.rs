use std::str::FromStr;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

mod embedded {
    refinery::embed_migrations!("src/migrations");
}

pub fn create_pool(url: &str) -> anyhow::Result<Pool> {
    let pg_config = tokio_postgres::Config::from_str(url)?;

    let mgr = Manager::from_config(
        pg_config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );

    let pool = Pool::builder(mgr).max_size(16).build()?;
    Ok(pool)
}

pub async fn run_migrations(pool: &Pool) -> anyhow::Result<()> {
    let mut client = pool.get().await?;
    embedded::migrations::runner()
        .run_async(&mut **client)
        .await?;
    Ok(())
}
