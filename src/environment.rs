use crate::{
    jwt::Jwt,
    pool::{self, Pool},
    settings::Settings,
};

#[derive(Clone)]
pub struct Environment {
    pub settings: Settings,
    pub jwt: Jwt,
    pub redis_pool: Pool,
}

impl Environment {
    pub async fn new(settings: Settings) -> anyhow::Result<Self> {
        let pool_mgr = pool::Manager::new(settings.redis.address);
        let redis_pool = pool::Pool::new(pool_mgr, settings.redis.pool_size);
        let jwt = Jwt::new(settings.channel.secret_key.as_str());
        Ok(Self {
            settings,
            jwt,
            redis_pool,
        })
    }
}
