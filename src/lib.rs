#![feature(duration_constructors)]
#![feature(error_generic_member_access)]
#![feature(iterator_try_collect)]

use crate::downloader::{DownType, Downloader, EXTRACTION_DB_ROOT, VIDEO_DL_NAME, path};
use crate::err::{SError, SResult, pretty_panic};
use crate::extractor::{
    extract_collections_from_root, extract_original_id, extract_video_from_collection,
};
use crate::global_config::GlobalConfig;
use simd_json::prelude::ArrayTrait;
use std::borrow::Cow;
use std::env;
use std::fs::{create_dir, read_dir};
use std::process::ExitCode;
use tracing::{info, trace};
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

mod downloader;
mod err;
mod extractor;
mod global_config;
mod utils;

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
    let collection_ids = load_top_collections(&mut downloader, &root_id)?;
    let mut all_videos = Vec::new();
    for collection_id in collection_ids {
        let videos = load_collection(&mut downloader, &collection_id)?;
        all_videos.extend(videos);
    }
    info!("extracted {} videos", all_videos.len());

    let mut ytdl_commands: Vec<String> = vec!["#!/bin/bash".into(), "set -eux".into()];
    for video_id in all_videos {
        if let Some(commands) = load_youtube_dl(&global_config, &video_id)? {
            ytdl_commands.extend(commands);
        }
    }
    let ytdl_script_path = path([EXTRACTION_DB_ROOT, "ytdl-scrape.sh"]);
    std::fs::write(&ytdl_script_path, ytdl_commands.join("\n"))
        .map_err(SError::io(&ytdl_script_path))?;
    info!(
        "wrote {} commands to {}",
        ytdl_commands.len(),
        ytdl_script_path.display()
    );

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

fn load_collection(downloader: &mut Downloader, collection_id: &str) -> SResult<Vec<String>> {
    let content = downloader.fetch(DownType::Collection, collection_id)?;
    extract_video_from_collection(content.body)
}

fn load_youtube_dl(global_config: &GlobalConfig, video_id: &str) -> SResult<Option<[String; 3]>> {
    /*
    Shockingly the backend Video ID is the public ID.
    This site `echo qrirybcre.bar.npprqb.gi | tr 'N-ZA-Mn-za-m' 'A-Za-z'` (rot13)
    is apparently simply a (super complex) subscription manager.

    I've paid for this content already so it's fine.
    */
    let video_root = path([EXTRACTION_DB_ROOT, VIDEO_DL_NAME, video_id]);
    if !video_root.exists() {
        create_dir(&video_root).map_err(SError::io(&video_root))?;
    }

    let children = read_dir(&video_root).map_err(SError::io(&video_root))?;
    let child_names: Vec<String> = children
        .map(|v| v.map(|v| v.file_name().to_string_lossy().to_string()))
        .try_collect()
        .map_err(SError::io(&video_root))?;

    let needs_download = if child_names.is_empty() {
        trace!("downloading new video {video_id}");
        true
    } else if child_names.iter().any(|v| v.contains("mp4")) {
        if child_names.iter().any(|v| v.contains(".part")) {
            panic!("remove old part data for {video_id}")
        } else {
            trace!("skipping already downloaded {video_id}");
            false
        }
    } else {
        panic!("not empty but doesn't contain mp4?? {video_id}")
    };

    if needs_download {
        let account_id = &global_config.bc_account_id;
        let final_url = format!(
            "https://players.brightcove.net/{account_id}/default_default/index.html?videoId={video_id}"
        );
        let full_root = video_root.canonicalize().unwrap();
        Ok(Some([
            format!("cd {}", full_root.display()),
            format!(
                "youtube-dl --write-info-json --write-thumbnail --verbose {final_url} | tee ytdl.log"
            ),
            "sleep 20".into(), // Be a nice scraper
        ]))
    } else {
        Ok(None)
    }
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
