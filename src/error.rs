use thiserror::Error;

#[derive(Error, Debug)]
pub enum RequestError {
    #[error("payload too large, limit: {limit:?}")]
    PayloadTooLarge { limit: usize },
}
