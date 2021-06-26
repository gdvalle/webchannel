#![warn(clippy::all)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::{Context, Result};
use clap::{App, Arg};
use std::env;
use std::path::Path;
use tracing::info;
use warp::Filter;

use webchannel::{environment::Environment, filters, metrics, problem, settings};

#[tokio::main]
async fn main() -> Result<()> {
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "webchannel=info");
    }

    tracing_subscriber::fmt::init();

    let args = App::new("webchannel")
        .version("0.0.1")
        .about("A conduit for pub/sub over websockets.")
        .arg(
            Arg::new("config_file")
                .short('c')
                .long("config-file")
                .value_name("[path/to/config.toml]")
                .multiple(false)
                .about("The config file to use, in TOML format.")
                .takes_value(true),
        )
        .get_matches();

    let settings = settings::Settings::new(args.value_of("config_file").map(|v| Path::new(v)))
        .context("failed to read config file")?;

    let mut cors_builder = warp::cors()
        .allow_methods(vec!["GET", "POST"])
        .allow_header("content-type")
        .allow_header("authorization");
    if settings.server.cors_allow_any_origin {
        cors_builder = cors_builder.allow_any_origin();
    } else if let Some(origins) = settings.server.cors_origins.clone() {
        cors_builder = cors_builder.allow_origins(origins.iter().map(|o| o.as_str()));
    }
    let cors = cors_builder.build();

    let env = Environment::new(settings.clone()).await?;

    let api = filters::webchannel(env.clone())
        .or(filters::health())
        .or(filters::metrics(env.settings.metrics))
        .recover(problem::unpack)
        .with(cors);

    let routes = api
        .with(warp::log("webchannel"))
        .with(warp::log::custom(metrics::warp_log_metrics));

    let listen_addr = settings.server.listen_address;
    info!("Starting server, listening on http://{}", listen_addr);
    warp::serve(routes).run(listen_addr).await;
    Ok(())
}
