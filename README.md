# webchannel

A simple HTTP API for publishing messages to WebSocket clients.

Basically an authed, unidirectional pipe connecting an HTTP publisher to a WebSocket subscriber, with no message persistence.

## What is this for?

Take a scenario where a server doing processing needs to notify a browser client when the task completes.

The client starts by requesting some work to be done, e.x. `POST /update-inventory`.

The server receives the request, and starts by using an API key to create a new `channel`.
Note the body is optional. If `channelId` is not provided a random URL-safe string will be generated.

```bash
curl --request POST --header "x-api-key: secret" --data '{"channelId": "user:1"}' http://localhost:8080/webchannel/v1/channels
```

It gets a response with a JWT:

```json
{
    "channelId": "user:1",
    "token":"<token>"
}
```

The server then issues a task to do the work. Minimally, it could just include the `token`, and the processing server could parse the channel ID out of the JWT claims. E.g. in JS: `JSON.parse(atob(token.split(".")[1])).cid`.  Note the token is only valid for the channel it was generated for.

Now that the task has started, the server replies to the browser with the `token`, and perhaps the `channelId`.

On the client, you can use the token to setup a subscriber:

```javascript
var ws = new WebSocket("ws://localhost:8080/webchannel/v1/channels/user:1?access_token=<token>")
ws.onmessage = m => m.data.text().then(JSON.parse).then(console.log)
```

Note `access_token` is passed as a URL parameter. That may not be so safe depending on your logging setup. The endpoint also accepts an `Authorization` header, but accepts the query param because the WebSocket browser API doesn't support that.

Back on the server, you can publish status messages:

```bash
curl --request POST --data '{"message": "Inventory update starting...", "percent": 0}' --header "Authorization: Bearer <token>" http://localhost:8080/webchannel/v1/channels/user:1

curl --request POST --data '{"message": "Added 14 banana breads", "percent": 50}' --header "Authorization: Bearer <token>" http://localhost:8080/webchannel/v1/channels/user:1

curl --request POST --data '{"message": "Inventory updated!", "percent": 100}' --header "Authorization: Bearer <token>" http://localhost:8080/webchannel/v1/channels/user:1
```

## What kind of data can I send over this thing?

_Any_ binary data is valid. The example here uses JSON, but this is essentially a raw pipe between an HTTP server, a Redis Pub/Sub channel, and a WebSocket client, and each simply relay that data without modification.

## Configuration

For options available, it's probably easiest to just look at `struct Settings` in [settings.rs](src/settings.rs).

Configuration can be managed through a TOML file or environment variables.

To specify a config file, use the CLI arg `--config-file` or `-c` with a path to a TOML file.

For environment vars, use the prefix `WC_`, and a double underscore to separate config areas from their keys. For example, to set `channel.ttl`, use `WC_CHANNEL__TTL=60`. This doesn't work for array types, so set those using a config file.

An example config file:

```toml
[server]
listen_address = "0.0.0.0:5000"
# Set CORS domains, or allow any.
cors_origins = ["https://example.com", "https://app.example.com"]
# cors_allow_any_origin = false

[channel]
# The secret used for JWT signing.
secret_key = "change-please"
# API keys used for creating channels, or publishing messages.
api_keys = ["foo", "bar"]
# TTL, in seconds, of the auth tokens generated for clients.
ttl = 86400

[metrics]
auth_enabled = true
auth_username = "chip"
auth_password = "munk"
```

## Metrics, Logging

Prometheus metrics are available at `/metrics`. Basic auth can be configured if needed.

Log levels are managed through the `RUST_LOG` environment variable.
To modify the server's log level try `RUST_LOG=webchannel=trace` to see _all_ server logs.
Use `RUST_LOG=debug` for debug across all modules.
See [env_logger](https://docs.rs/env_logger/) for more detail.

## Disclaimer

This is basically a weekend project that seemed simple enough to implement in Rust for fun. I decided to write a README to perhaps convince myself this has some kind of use. I can't guarantee any support, help, or maintenance, but it is MIT licensed, so feel free to do whatever with it.
