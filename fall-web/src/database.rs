use crate::endpoints::CheckHealth;
use crate::error::FallError;
use crate::PoolConfig;
use diesel::{
    connection::Connection,
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use fall_log::info;
use r2d2::PooledConnection;
use serde::Deserialize;
use std::fmt::Debug;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DatabaseConfig {
    url: String,
    pool: Option<PoolConfig>,
}

#[derive(Clone)]
pub struct DatabaseConn(pub Pool<ConnectionManager<PgConnection>>);

impl DatabaseConn {
    fn get_conn(&self) -> Result<PooledConnection<ConnectionManager<PgConnection>>, FallError> {
        Ok(self.0.get()?)
    }
}

impl CheckHealth for DatabaseConn {
    fn check(&self) -> Result<(), FallError> {
        Ok(self.get_conn()?.begin_test_transaction()?)
    }
}

impl DatabaseConfig {
    pub fn init(&self) -> Result<DatabaseConn, FallError> {
        info!("Init database...");
        Ok(self
            .pool
            .as_ref()
            .map(|p| {
                Pool::builder()
                    .max_size(p.max_size.unwrap_or(10))
                    .min_idle(p.min_idle)
                    .max_lifetime(p.max_lifetime)
                    .idle_timeout(p.idle_timeout)
                    .connection_timeout(p.connection_timeout.unwrap_or(Duration::from_secs(30)))
            })
            .unwrap_or_else(Pool::builder)
            .build(ConnectionManager::new(&self.url))
            .map(DatabaseConn)?)
    }
}
