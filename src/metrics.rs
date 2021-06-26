use prometheus::{opts, HistogramVec, IntCounter, IntCounterVec, IntGauge};

lazy_static! {
    pub static ref MESSAGES_PUBLISHED: IntCounter = register_int_counter!(
        "webchannel_messages_published_total",
        "Total number of messages published."
    )
    .unwrap();
    pub static ref MESSAGES_PUBLISHED_BYTES: IntCounter = register_int_counter!(
        "webchannel_messages_published_bytes_total",
        "Total bytes of messages published."
    )
    .unwrap();
    pub static ref MESSAGES_SENT: IntCounter = register_int_counter!(
        "webchannel_messages_sent_total",
        "Total number of messages sent to subscribers."
    )
    .unwrap();
    pub static ref WEBSOCKET_MESSAGES_RECEIVED: IntCounter = register_int_counter!(
        "webchannel_websocket_messages_received_total",
        "Total number of messages received over a websocket connection."
    )
    .unwrap();
    pub static ref MESSAGES_SENT_BYTES: IntCounter = register_int_counter!(
        "webchannel_messages_sent_bytes_total",
        "Total bytes of messages sent to subscribers."
    )
    .unwrap();
    pub static ref MESSAGE_SEND_ERRORS: IntCounter = register_int_counter!(
        "webchannel_message_send_errors_total",
        "Count of errors encountered while sending to clients."
    )
    .unwrap();
    pub static ref USERS_CONNECTED: IntGauge = register_int_gauge!(
        "webchannel_users_connected",
        "Count of users currently connected to websockets."
    )
    .unwrap();
    pub static ref REDIS_CONNECTIONS_CREATED: IntCounterVec = register_int_counter_vec!(
        opts!(
            "webchannel_redis_connections_created_total",
            "Total number of redis connections created."
        ),
        &["pooled"]
    )
    .unwrap();
    pub static ref REDIS_CONNECTION_ERRORS: IntCounter = register_int_counter!(
        "webchannel_redis_connection_errors_total",
        "Total errors encountered while creating a connection."
    )
    .unwrap();
    pub static ref REDIS_PUBLISH_ERRORS: IntCounter = register_int_counter!(
        "webchannel_redis_publish_errors_total",
        "Total errors encountered while publishing a message."
    )
    .unwrap();
    pub static ref REDIS_SUBSCRIBE_ERRORS: IntCounter = register_int_counter!(
        "webchannel_redis_subscribe_errors_total",
        "Total errors encountered while subscribing to a channel."
    )
    .unwrap();
    pub static ref REDIS_SUBSCRIBE_UNEXPECTED_MESSAGE_TYPES: IntCounter = register_int_counter!(
        "webchannel_redis_subscribe_unexpected_message_types_total",
        "Total errors encountered while subscribing to a channel."
    )
    .unwrap();
    pub static ref HTTP_RESPONSE_TIME: HistogramVec = register_histogram_vec!(
        "webchannel_http_response_time_seconds",
        "Response duration by handler, in seconds.",
        &["endpoint", "method"],
        prometheus::DEFAULT_BUCKETS.to_vec()
    )
    .unwrap();
    pub static ref HTTP_RESPONSES: IntCounterVec = register_int_counter_vec!(
        opts!(
            "webchannel_http_responses_total",
            "HTTP response codes by handler and status."
        ),
        &["endpoint", "status"]
    )
    .unwrap();
}

// Known path segments. Just a simple way of naming handlers for metrics while
// avoiding cardinality issues.
static METRIC_PATH_SEGMENTS: [&str; 8] = [
    "",
    "webchannel",
    "v1",
    "channels",
    "publish",
    "subscribe",
    "healthz",
    "metrics",
];
static PLACEHOLDER_SEGMENT: &str = "*";

pub fn warp_log_metrics(info: warp::log::Info) {
    let endpoint = info
        .path()
        .split('/')
        .map(|segment| match METRIC_PATH_SEGMENTS.contains(&segment) {
            true => segment,
            false => PLACEHOLDER_SEGMENT,
        })
        .collect::<Vec<&str>>()
        .join("/");
    let method = info.method();
    let status = info.status();

    HTTP_RESPONSE_TIME
        .with_label_values(&[endpoint.as_str(), method.as_str()])
        .observe(info.elapsed().as_secs_f64());

    HTTP_RESPONSES
        .with_label_values(&[endpoint.as_str(), status.as_str()])
        .inc();
}
