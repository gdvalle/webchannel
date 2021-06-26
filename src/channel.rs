use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct CreateChannelRequest {
    #[serde(rename = "channelId")]
    pub channel_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChannelToken {
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub token: String,
}
