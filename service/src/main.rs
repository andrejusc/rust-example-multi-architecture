use std::{env, error::Error, str::FromStr};

use async_graphql::{http::GraphiQLSource, EmptyMutation, EmptySubscription, Object, Schema};
use async_graphql_poem::GraphQL;
use config::{Config, File, FileFormat};
use poem::{listener::TcpListener, web::Html, get, Route, Server, handler};
use tracing::{info, metadata::LevelFilter};
use tracing_subscriber::{prelude::*, Layer, filter};

mod custom_tracing_layer;
use custom_tracing_layer::CustomTracingLayer;

struct MyQuery;

#[Object]
impl MyQuery {
    async fn howdy(&self) -> &'static str {
        "partner"
    }

    async fn details(&self) -> &'static str {
        // Here could call some other backend/DB to get data
        "Your requested details"
    }
}

#[handler]
async fn graphiql() -> Html<String> {
    Html(GraphiQLSource::build().finish())
}

/// Makes merged logging config. WIll panic if ENVROLE env variable is not set.
fn make_log_config() -> Config {
    let env_role = env::var("ENVROLE").unwrap();
    Config::builder()
        .add_source(File::new("env/logging", FileFormat::Yaml))
        .add_source(File::new(
            ("env/logging-".to_string() + &env_role).as_str(),
            FileFormat::Yaml,
        ))
        .build()
        .unwrap()
}

/// Makes merged main config. WIll panic if ENVROLE env variable is not set.
fn make_config() -> Config {
    let env_role = env::var("ENVROLE").unwrap();
    Config::builder()
        .add_source(File::new("env/service", FileFormat::Yaml))
        .add_source(File::new(
            ("env/service-".to_string() + &env_role).as_str(),
            FileFormat::Yaml,
        ))
        .build()
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let log_config = make_log_config();
    // Enables spans and events with levels `INFO` and below:
    // let level_filter = LevelFilter::INFO;
    let level_str = log_config.get_string("level").unwrap();
    let level_filter: LevelFilter = match FromStr::from_str(&level_str) {
        Ok(filter) => filter,
        Err(error) => {
            panic!("Problem parsing log level: {error:?}, supplied level: {level_str}");
        }
    };
    // Set up how `tracing-subscriber` will deal with tracing data.
    tracing_subscriber::registry()
        .with(
            CustomTracingLayer
                .with_filter(level_filter)
                .with_filter(filter::filter_fn(|metadata| {
                    metadata.target().eq("service")
                })),
        )
        .init();

    info!(
        "Starting logging at level: {}, for env role: {}",
        &level_str,
        env::var("ENVROLE").unwrap()
    );
    let config = make_config();

    // create the schema
    let schema = Schema::build(MyQuery, EmptyMutation, EmptySubscription).finish();
    // If you want to print resulting schema definition
    // println!("schema: {}", schema.sdl());

    let app = Route::new().at("/", get(graphiql).post(GraphQL::new(schema)));
    let port = config.get_int("port").unwrap();
    info!("GraphiQL: http://localhost:{}", port);
    // start the http server
    Server::new(TcpListener::bind(format!("0.0.0.0:{port}")))
        .run(app)
        .await?;

    Ok(())
}
