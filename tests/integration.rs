use reqwest::StatusCode;
use std::net::SocketAddr;

use crate::util::CHANNEL_SECRET;

fn parse_token(token: &str, validate_expiry: bool) -> anyhow::Result<()> {
    let secret = biscuit::jws::Secret::bytes_from_str(CHANNEL_SECRET);
    let token = biscuit::JWT::<biscuit::RegisteredClaims, biscuit::Empty>::new_encoded(&token)
        .into_decoded(&secret, biscuit::jwa::SignatureAlgorithm::HS256)?;

    let claims = token.payload()?;
    if validate_expiry {
        claims
            .registered
            .validate_exp(biscuit::Validation::Validate(
                biscuit::TemporalOptions::default(),
            ))?;
    }
    Ok(())
}

fn v1_url(addr: &SocketAddr, path: &str) -> String {
    format!("http://{}/webchannel/v1{}", addr, path)
}

fn send_message(
    addr: &SocketAddr,
    channel_id: &str,
    message: &str,
    token: &str,
) -> Result<reqwest::blocking::Response, reqwest::Error> {
    let client = reqwest::blocking::Client::new();
    let message = message.to_string();
    client
        .post(v1_url(&addr, format!("/channels/{}", channel_id).as_str()))
        .body(message)
        .header("authorization", format!("Bearer {}", token))
        .send()
}

fn connect_subscriber(addr: &SocketAddr, channel_id: &str, token: &str) -> http::Request<()> {
    http::Request::builder()
        .method("GET")
        .uri(format!(
            "ws://{}/webchannel/v1/channels/{}",
            addr, channel_id
        ))
        .header("authorization", format!("Bearer {}", token))
        .body(())
        .expect("Failed to build subscriber request")
}

server_test!(test_channel, "", |addr: SocketAddr| {
    let client = reqwest::blocking::Client::new();
    let api_key = "foo";

    // Create a random channel
    let response = client
        .post(v1_url(&addr, "/channels"))
        .header("x-api-key", api_key)
        .body("{}")
        .send()
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value = response.json().unwrap();
    let token = json["token"].as_str().unwrap();
    assert!(parse_token(&token, true).is_ok());
    let channel_id = json["channelId"].as_str().unwrap();
    assert!(channel_id.len() > 5);

    // Create a named channel
    let response = client
        .post(v1_url(&addr, "/channels"))
        .header("x-api-key", api_key)
        .json(&serde_json::json!({
            "channelId": "foo"
        }))
        .send()
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value = response.json().unwrap();
    let token = json["token"].as_str().unwrap();
    assert!(parse_token(&token, true).is_ok());
    let channel_id = json["channelId"].as_str().unwrap();
    assert_eq!(channel_id, "foo", "{:?}", json);

    // Publish a message, without a token
    let response = client
        .post(v1_url(&addr, "/channels/foo"))
        .header("x-api-key", api_key)
        .body("hello")
        .send()
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Publish a message, with a valid token
    let response = send_message(&addr, "foo", "hello", &token).unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Connect a subscriber, without auth
    let request = connect_subscriber(&addr, "foo", "");
    let result = tungstenite::connect(request);
    assert!(result.is_err(), "Subscriber allowed without auth");

    // Connect a subscriber, with auth, but without channel auth
    let request = connect_subscriber(&addr, "bar", &token);
    let result = tungstenite::connect(request);
    assert!(result.is_err(), "Subscriber used unpermitted channel");

    // Connect a subscriber, with valid auth header, and valid channel
    let request = connect_subscriber(&addr, "foo", &token);
    let (mut socket, response) = tungstenite::connect(request).unwrap();
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    assert!(socket.close(None).is_ok());

    // Connect a subscriber, with valid access_token param, and valid channel
    let request = http::Request::builder()
        .method("GET")
        .uri(format!(
            "ws://{}/webchannel/v1/channels/{}?access_token={}",
            addr, channel_id, token
        ))
        .body(())
        .expect("Failed to build subscriber request");
    let (mut socket, response) = tungstenite::connect(request).unwrap();
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);

    // Publish a message for the subscriber
    let response = send_message(&addr, "foo", "hello on foo", &token).unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Read the message
    let msg = socket.read_message().unwrap();
    assert_eq!(
        msg,
        tungstenite::Message::Binary("hello on foo".as_bytes().to_vec())
    );
});

server_test!(
    test_channel_api_key,
    // With a config specifying API keys
    "tests/settings/api_keys.toml",
    |addr: SocketAddr| {
        let client = reqwest::blocking::Client::new();

        // If I request a new channel token without auth
        let response = client
            .post(v1_url(&addr, "/channels"))
            .body("{}")
            .send()
            .unwrap();

        // I expect to be denied
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // If I request a new channel with an invalid key
        let response = client
            .post(v1_url(&addr, "/channels"))
            .body("{}")
            .header("x-api-key", "invalid-key")
            .send()
            .unwrap();

        // I expect to be denied
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // If I publish a message without auth
        let response = client
            .post(v1_url(&addr, "/channels/foo"))
            .body("{}")
            .send()
            .unwrap();

        // I expect to be denied
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let api_keys = &["foo", "bar"];
        for api_key in api_keys {
            // If I request a channel with a valid key
            let response = client
                .post(v1_url(&addr, "/channels"))
                .body("{}")
                .header("x-api-key", *api_key)
                .send()
                .unwrap();

            // I expect a successful response
            assert_eq!(response.status(), StatusCode::OK);

            // If I publish a message with a valid key
            let response = client
                .post(v1_url(&addr, "/channels/foo"))
                .body("{}")
                .header("x-api-key", *api_key)
                .send()
                .unwrap();

            // I expect a successful response
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }
    }
);
