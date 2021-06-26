use crate::{auth, channel, environment::Environment, handlers, metrics, problem, settings};
use serde::{Deserialize, Serialize};
use tracing::error;
use tracing::{debug, trace};
use warp::{Filter, Rejection, Reply};

const MAX_MESSAGE_SIZE: usize = 1024 * 512;
const BEARER: &str = "Bearer ";

#[derive(Deserialize, Serialize)]
struct AuthQuery {
    access_token: String,
}

pub fn webchannel(
    environment: Environment,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let with_env = warp::any().map(move || environment.clone());

    let api_key_auth = warp::header::optional("x-api-key")
        .and(with_env.clone())
        .and_then(|header_value: Option<String>, e: Environment| async move {
            let header_value = header_value.unwrap_or_default();
            let valid = match e.settings.channel.api_keys {
                Some(api_keys) => api_keys.iter().any(|api_key| &header_value == api_key),
                None => true,
            };
            if valid {
                trace!("API key valid");
                Ok(header_value)
            } else {
                trace!("API key invalid");
                Err(problem::build(auth::AuthError::InvalidCredentials))
            }
        });

    let auth_header_token = warp::header("authorization").map(|v: String| {
        let (_bearer, token) = v.split_once(BEARER).unwrap_or(("", ""));
        token.to_string()
    });

    let auth_query_token = warp::query::<AuthQuery>().map(|q: AuthQuery| q.access_token);

    let any_auth_token = auth_header_token.or(auth_query_token).unify();

    let validate_token = |token: String, env: Environment| async move {
        match env.jwt.decode(token.as_str()) {
            Ok(claims) => Ok(claims),
            Err(_) => Err(problem::build(auth::AuthError::InvalidCredentials)),
        }
    };

    let valid_auth_header = auth_header_token
        .or_else(|_r| async { Err(problem::build(auth::AuthError::InvalidCredentials)) })
        .and(with_env.clone())
        .and_then(validate_token);

    let any_token_auth = any_auth_token
        .or_else(|_r| async { Err(problem::build(auth::AuthError::InvalidCredentials)) })
        .and(with_env.clone())
        .and_then(validate_token);

    let publish = channel_param()
        .and(warp::path::end())
        .and(warp::post())
        .and(with_limited_body(MAX_MESSAGE_SIZE))
        .and(with_env.clone())
        .and(valid_auth_header.or(api_key_auth.clone()))
        .and_then(|channel: String, body, env, _auth| async move {
            handlers::publish(channel.as_str(), body, env)
                .await
                .map_err(problem::build)
        });

    let subscribe = channel_param()
        // let subscribe = warp::path::param::<String>()
        .and(warp::path::end())
        .and(any_token_auth)
        .and(warp::ws())
        .and(with_env.clone())
        .and_then(
            |channel_id: String,
             claims: biscuit::ClaimsSet<auth::Claims>,
             ws: warp::ws::Ws,
             env| async move {
                if channel_id == claims.private.cid {
                    trace!("Channel matches claim, allowing upgrade");
                    let reply = ws.max_message_size(MAX_MESSAGE_SIZE as usize).on_upgrade(
                        move |websocket| async move {
                            metrics::USERS_CONNECTED.inc();
                            if let Err(e) = handlers::subscribe(&channel_id, env, websocket).await {
                                error!("Subscribe error on channel {:?}: {:?}", &channel_id, e);
                            }
                            metrics::USERS_CONNECTED.dec();
                        },
                    );
                    Ok(reply)
                } else {
                    debug!(
                        "Requested channel and claim mismatch: requested: {:?}, claim: {:?}",
                        channel_id, claims.private.cid
                    );
                    Err(problem::build(auth::AuthError::InvalidCredentials))
                }
            },
        );

    let create_channel = warp::path::end()
        .and(warp::post())
        .and(api_key_auth)
        .and(with_env)
        // warp::body::content_length_limit throws a Rejection when content-length is unset.
        // Since a payload is optional on this request, allow it go unset.
        .and(with_limited_body(1024 * 16))
        .and_then(move |_auth_header, env, body: Vec<u8>| async move {
            let req = match body.len() {
                0 => channel::CreateChannelRequest::default(),
                _ => match serde_json::from_slice(body.as_slice()) {
                    Ok(req) => req,
                    Err(e) => {
                        return Err(problem::build(e));
                    }
                },
            };
            handlers::create_channel(env, req)
                .await
                .map_err(problem::build)
        });

    warp::path("webchannel")
        .and(warp::path("v1"))
        .and(warp::path("channels"))
        .and(publish.or(subscribe).or(create_channel))
}

fn channel_param() -> impl Filter<Extract = (String,), Error = Rejection> + Copy {
    warp::path::param::<String>()
}

async fn limited_body(
    mut stream: impl futures::Stream<Item = Result<impl warp::Buf, warp::Error>> + Unpin,
    max_bytes: &usize,
) -> anyhow::Result<Vec<u8>> {
    use futures::TryStreamExt;
    let mut body: Vec<u8> = vec![];
    while let Some(mut buf) = stream.try_next().await? {
        trace!("Reading buf, size: {:?}", buf.remaining());
        while buf.has_remaining() {
            let chunk = buf.chunk();
            let chunk_size = chunk.len();
            body.extend_from_slice(chunk);
            buf.advance(chunk_size);
            if &body.len() > max_bytes {
                return Err(
                    crate::error::RequestError::PayloadTooLarge { limit: *max_bytes }.into(),
                );
            }
        }
    }
    Ok(body)
}

fn with_limited_body(
    max_bytes: usize,
) -> impl Filter<Extract = (Vec<u8>,), Error = warp::Rejection> + Clone {
    warp::filters::body::stream()
        .and_then(move |s| async move { limited_body(s, &max_bytes).await.map_err(problem::build) })
}

pub fn health() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("healthz")
        .and(warp::get())
        .and(warp::path::end())
        .and_then(handlers::health)
}

fn check_metrics_auth(settings: settings::Metrics, auth_header: Option<String>) -> bool {
    if !settings.auth_enabled {
        return true;
    }
    match auth_header {
        Some(h) => h.as_bytes() == settings.make_basic_auth_header().as_bytes(),
        None => false,
    }
}

pub fn metrics(
    settings: settings::Metrics,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let with_settings = warp::any().map(move || settings.clone());
    warp::path!("metrics")
        .and(warp::get())
        .and(warp::path::end())
        .and(with_settings.and(warp::header::optional("authorization")))
        .and_then(
            move |settings: settings::Metrics, auth_header: Option<String>| async move {
                if !check_metrics_auth(settings, auth_header) {
                    return Err(problem::build(auth::AuthError::InvalidCredentials));
                }
                handlers::metrics().await.map_err(problem::build)
            },
        )
}
