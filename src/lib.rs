#![feature(duration_constructors)]
#![feature(error_generic_member_access)]

use crate::downloader::{DownType, Downloader};
use crate::err::{SResult, pretty_panic};
use crate::extractor::{extract_collections_from_root, extract_original_id};
use crate::global_config::GlobalConfig;
use std::env;
use std::process::ExitCode;
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

mod downloader;
mod err;
mod extractor;
mod global_config;

pub fn start_scraper() -> ExitCode {
    init_logging();
    if let Err(e) = _start_scraper() {
        pretty_panic(e);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn _start_scraper() -> SResult<()> {
    let global_config = GlobalConfig::load()?;
    let mut downloader = Downloader::init(&global_config);
    DownType::mkdirs();

    let root_id = load_root_collection_id(&mut downloader)?;
    let collection_urls = load_top_collections(&mut downloader, &root_id)?;

    Ok(())
}

fn load_root_collection_id(downloader: &mut Downloader) -> SResult<String> {
    let content = downloader.fetch(DownType::HTML, "")?;
    extract_original_id(&content.body)
}

fn load_top_collections(downloader: &mut Downloader, root_id: &str) -> SResult<Vec<String>> {
    let content = downloader.fetch(DownType::Page, root_id)?;
    extract_collections_from_root(content.body)
}

fn init_logging() {
    let default_env = "trace,\
    reqwest::blocking::wait=DEBUG,\
    reqwest::blocking::client=DEBUG,\
    hyper_util::client::legacy::pool=DEBUG,\
    selectors::matching=INFO,\
    reqwest::connect=DEBUG,\
    hyper_util::client::legacy::client=DEBUG,\
    html5ever=INFO";
    // let default_env = "trace";

    let subscriber = Registry::default();

    let env_var = env::var(EnvFilter::DEFAULT_ENV).unwrap_or_else(|_| default_env.into());
    let env_layer = EnvFilter::builder().parse(env_var).expect("bad env");
    let subscriber = subscriber.with(env_layer);

    let filter_layer = Layer::default().compact();
    let subscriber = subscriber.with(filter_layer);

    subscriber.init()
}
