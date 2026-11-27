mod acquire;
mod eq;
mod artimg;
mod artwork;
mod backend;
mod cache;
mod cast;
mod control;
mod history;
mod model;
mod mpris;
mod performer;
mod places;
mod player;
mod podcasts;
mod radio;
mod sync;
mod taste;
mod ui;
mod unify;
mod urlfix;
mod ytm;

use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    ui::run_app()
}
