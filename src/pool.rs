use crate::metrics;
use async_trait::async_trait;
use redis_async::client::{paired_connect, PairedConnection};
use redis_async::error::Error as RedisError;
use redis_async::{resp::RespValue, resp_array};
use std::net::SocketAddr;
use tracing::debug;

pub type Pool = deadpool::managed::Pool<Manager>;
type RecycleResult = deadpool::managed::RecycleResult<RedisError>;

pub struct Manager {
    addr: SocketAddr,
}

impl Manager {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

#[async_trait]
impl deadpool::managed::Manager for Manager {
    type Type = PairedConnection;
    type Error = RedisError;

    async fn create(&self) -> Result<PairedConnection, RedisError> {
        debug!("Creating new redis connection");
        match paired_connect(self.addr).await {
            Ok(conn) => {
                metrics::REDIS_CONNECTIONS_CREATED
                    .with_label_values(&["true"])
                    .inc();
                Ok(conn)
            }
            Err(e) => {
                metrics::REDIS_CONNECTION_ERRORS.inc();
                return Err(e);
            }
        }
    }

    async fn recycle(&self, conn: &mut PairedConnection) -> RecycleResult {
        let _: RespValue = conn.send(resp_array!["PING"]).await?;
        Ok(())
    }
}
