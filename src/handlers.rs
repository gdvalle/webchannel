use crate::{
    auth,
    channel::{ChannelToken, CreateChannelRequest},
    environment::Environment,
    metrics,
};
use anyhow::Context;
use chrono::{prelude::*, Duration};
use futures::{
    select,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use prometheus::{Encoder as PrometheusEncoder, TextEncoder};
use redis_async::{client::pubsub::PubsubStream, resp::RespValue, resp_array};
use std::convert::{Infallible, TryFrom};
use tracing::{debug, error, trace, warn};
use warp::{
    http,
    ws::{Message, WebSocket},
    Reply,
};

fn make_channel_key(channel_id: &str) -> String {
    format!("wc:channel:{}", channel_id)
}

pub async fn health() -> Result<impl Reply, Infallible> {
    Ok("OK")
}

pub async fn metrics() -> anyhow::Result<impl Reply> {
    let mut buf = vec![];
    let encoder = TextEncoder::new();
    encoder.encode(&prometheus::gather(), &mut buf)?;
    let output = String::from_utf8(buf)?;
    Ok(output)
}

pub async fn publish(
    channel_id: &str,
    body: Vec<u8>,
    env: Environment,
) -> anyhow::Result<impl Reply> {
    let connection = env
        .redis_pool
        .get()
        .await
        .context("Failed to get redis connection from pool")?;

    let body_size = body.len();
    let resp = resp_array!["PUBLISH", make_channel_key(&channel_id), body];
    let _clients: RespValue = connection
        .send(resp)
        .await
        .context("Failed to send publish command")?;

    metrics::MESSAGES_PUBLISHED.inc();
    metrics::MESSAGES_PUBLISHED_BYTES.inc_by(u64::try_from(body_size).unwrap());

    Ok(warp::reply::with_status(
        warp::reply(),
        http::StatusCode::NO_CONTENT,
    ))
}

async fn handle_channel_message(
    ws_tx: &mut SplitSink<warp::ws::WebSocket, warp::ws::Message>,
    redis_result: Result<RespValue, redis_async::error::Error>,
) -> anyhow::Result<()> {
    let resp_value = redis_result.context("Error receiving channel message")?;

    match resp_value {
        RespValue::BulkString(v) => match ws_tx.send(warp::filters::ws::Message::binary(v)).await {
            Ok(_) => metrics::MESSAGES_SENT.inc(),
            Err(e) => {
                warn!("Error sending websocket message: {:?}", e);
                metrics::MESSAGE_SEND_ERRORS.inc();
                return Err(anyhow::anyhow!(e));
            }
        },
        _ => {
            metrics::REDIS_SUBSCRIBE_UNEXPECTED_MESSAGE_TYPES.inc();
            error!("Received unexpected redis type, ignoring");
        }
    }
    Ok(())
}

async fn relay_messages(
    ws_tx: &mut SplitSink<WebSocket, Message>,
    ws_rx: SplitStream<WebSocket>,
    messages: PubsubStream,
) -> anyhow::Result<()> {
    // select macro requires these to be fused.
    let mut rx = ws_rx.fuse();
    let mut msgs = messages.fuse();

    loop {
        // Poll for client disconnects or pub/sub messages.
        // Even though we don't use client messages, we must poll for disconnects.
        let result = select! {
            chan_msg = msgs.next() => Ok(chan_msg),
            client_msg = rx.next() => Err(client_msg),
        };
        match result {
            Ok(chan_msg) => {
                if let Some(redis_result) = chan_msg {
                    if handle_channel_message(ws_tx, redis_result).await.is_err() {
                        break;
                    }
                }
            }
            Err(client_rx_select) => {
                if let Some(client_rx_result) = client_rx_select {
                    match client_rx_result {
                        Ok(_client_msg) => {
                            metrics::WEBSOCKET_MESSAGES_RECEIVED.inc();
                            debug!("Received client message, aborting");
                            break;
                        }
                        Err(e) => {
                            debug!("WebSocket connection error: {:?}", e);
                            break;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn subscribe(
    channel_id: &str,
    env: Environment,
    websocket: WebSocket,
) -> anyhow::Result<()> {
    trace!("New subscriber on channel {:?}", channel_id);
    let (mut ws_tx, ws_rx) = websocket.split();

    let redis_addr = env.settings.redis.address;
    let rd_client = match redis_async::client::pubsub_connect(redis_addr)
        .await
        .context("Failed connecting to redis")
    {
        Ok(conn) => {
            metrics::REDIS_CONNECTIONS_CREATED
                .with_label_values(&["false"])
                .inc();
            conn
        }
        Err(e) => {
            metrics::REDIS_CONNECTION_ERRORS.inc();
            let _ = ws_tx.close().await;
            return Err(e);
        }
    };

    let messages = match rd_client
        .subscribe(make_channel_key(&channel_id).as_str())
        .await
        .context("Failed subscribing to redis channel")
    {
        Ok(stream) => stream,
        Err(e) => {
            let _ = ws_tx.close().await;
            return Err(e);
        }
    };

    let result = relay_messages(&mut ws_tx, ws_rx, messages).await;
    let _ = ws_tx.close().await;
    result
}

pub async fn create_channel(
    env: Environment,
    request: CreateChannelRequest,
) -> anyhow::Result<impl Reply> {
    let channel_id = match request.channel_id {
        Some(cid) => cid,
        None => nanoid::nanoid!(),
    };

    trace!("Creating token for channel {:?}", channel_id);
    let token = env.jwt.encode(
        auth::Claims {
            cid: channel_id.clone(),
        },
        Utc::now() + Duration::seconds(env.settings.channel.ttl as i64),
    )?;

    Ok(warp::reply::json(&ChannelToken { channel_id, token }))
}
