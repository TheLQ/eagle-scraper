#![feature(duration_constructors)]
#![feature(error_generic_member_access)]
#![feature(iterator_try_collect)]

use crate::downloader::{
    BROWSE_NAME, DownType, Downloader, EXTRACTION_DB_ROOT, VIDEO_DL_NAME, path,
};
use crate::err::{SError, SResult, pretty_panic};
use crate::extractor::{
    ExtractedThing, ThingType, extract_collections_from_root, extract_original_id,
    extract_things_from_collection,
};
use crate::global_config::GlobalConfig;
use simd_json::prelude::{ArrayTrait, ValueObjectAccessAsScalar};
use std::env;
use std::fs::{create_dir, read_dir};
use std::process::ExitCode;
use tracing::{info, trace, warn};
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

    let mut all_videos: Vec<ExtractedThing> = Vec::new();
    let mut seen_ids = Vec::new();

    let mut spider = vec![ExtractedThing {
        next_type: ThingType::Page,
        next_id: load_root_collection_id(&mut downloader)?,
        title: "from_root".into(),
    }];
    while let Some(cur_thing) = spider.pop() {
        if seen_ids.contains(&cur_thing.next_id) {
            // Apparently videos exist in multiple collections
            assert_eq!(cur_thing.next_type, ThingType::Video);

            warn!("skipping seen id {}", cur_thing.next_id);
            continue;
        }
        seen_ids.push(cur_thing.next_id.clone());

        match cur_thing.next_type {
            ThingType::Page => {
                let collections = load_collections_from_page(&mut downloader, &cur_thing.next_id)?;
                spider.extend(collections);
            }
            ThingType::Collection => {
                let nexts = load_collection(&mut downloader, &cur_thing.next_id)?;
                spider.extend(nexts)
            }
            ThingType::Video => {
                all_videos.push(cur_thing);
            }
        }
    }
    info!("extracted {} videos", all_videos.len());

    all_videos.retain(|v| !global_config.missing_videos.contains(&v.next_id));

    let mut ytdl_commands: Vec<String> = vec!["#!/bin/bash".into(), "set -eux".into()];
    for video_id in &all_videos {
        if let Some(commands) = load_youtube_dl(&global_config, video_id)? {
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

    if ytdl_commands.len() == 2 {
        for video_id in &all_videos {
            synth_browse_dir(&video_id.next_id)?;
        }
    } else {
        info!("skip browse synth")
    }

    Ok(())
}

fn load_root_collection_id(downloader: &mut Downloader) -> SResult<String> {
    let content = downloader.fetch(DownType::HTML, "")?;
    extract_original_id(&content.body)
}

fn load_collections_from_page(
    downloader: &mut Downloader,
    root_id: &str,
) -> SResult<Vec<ExtractedThing>> {
    let content = downloader.fetch(DownType::Page, root_id)?;
    extract_collections_from_root(content.body)
}

fn load_collection(
    downloader: &mut Downloader,
    collection_id: &str,
) -> SResult<Vec<ExtractedThing>> {
    let content = downloader.fetch(DownType::Collection, collection_id)?;
    extract_things_from_collection(content.body)
}

fn load_youtube_dl(
    global_config: &GlobalConfig,
    video_thing: &ExtractedThing,
) -> SResult<Option<[String; 4]>> {
    let video_id = &video_thing.next_id;
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
    let mut child_names: Vec<String> = children
        .map(|v| v.map(|v| v.file_name().to_string_lossy().to_string()))
        .try_collect()
        .map_err(SError::io(&video_root))?;

    let needs_download = if child_names.is_empty() {
        trace!("downloading new video {video_id}");
        true
    } else if child_names.len() == 1 && child_names[0] == "ytdl.log" {
        trace!("retry {video_id}");
        true
    } else {
        let Some(pos) = child_names.iter().position(|v| v.ends_with(".mp4")) else {
            panic!("missing mp4")
        };
        child_names.remove(pos);

        let Some(pos) = child_names.iter().position(|v| v.ends_with(".info.json")) else {
            panic!("missing mp4")
        };
        child_names.remove(pos);

        let Some(pos) = child_names.iter().position(|v| v.ends_with(".jpg")) else {
            panic!("missing mp4")
        };
        child_names.remove(pos);

        // maybe exists
        child_names.retain(|v| v != "ytdl.log");

        if child_names.is_empty() {
            false
        } else {
            panic!("unknown remaining files {}", child_names.join(","))
        }
    };

    if needs_download {
        let account_id = &global_config.bc_account_id;
        let final_url = format!(
            "https://players.brightcove.net/{account_id}/default_default/index.html?videoId={video_id}"
        );
        let full_root = video_root.canonicalize().unwrap();
        Ok(Some([
            format!("cd {}", full_root.display()),
            format!("echo \"{}\"", video_thing.title),
            format!(
                "youtube-dl --write-info-json --write-thumbnail --verbose {final_url} 2>&1 | tee ytdl.log"
            ),
            "sleep 20".into(), // Be a nice scraper
        ]))
    } else {
        Ok(None)
    }
}

fn synth_browse_dir(video_id: &str) -> SResult<()> {
    let video_root = path([EXTRACTION_DB_ROOT, VIDEO_DL_NAME, video_id]);
    if !video_root.exists() {
        panic!("missing video dl {video_id}")
    }

    let Some(info_path) = read_dir(&video_root)
        .map_err(SError::io(&video_root))?
        .map(|e| e.unwrap())
        .find(|e| e.file_name().to_string_lossy().ends_with(".info.json"))
        .map(|e| e.path())
    else {
        panic!("missing info.json in {}", video_root.display())
    };
    let mut info_raw = std::fs::read(&info_path).map_err(SError::io(info_path))?;
    let info_json = simd_json::to_borrowed_value(&mut info_raw).unwrap();

    let upload_date = info_json.get_str("upload_date").expect("upload_date");
    let upload_year = &upload_date[0..4];
    let upload_month = &upload_date[4..6];
    let upload_day = &upload_date[6..];
    let title = info_json.get_str("fulltitle").expect("fulltitle");

    let mut final_name = format!("{upload_year}-{upload_month}-{upload_day} {title}");
    if final_name.contains(":") {
        trace!("removing colon from {final_name}");
        final_name = final_name.replace(":", " -");
    }
    let final_path = path([EXTRACTION_DB_ROOT, BROWSE_NAME, &final_name]);

    let needs_create = if final_path.is_symlink() {
        trace!("skipping existing");
        false
    } else if final_path.exists() {
        panic!("unknown existing {}", final_path.display());
    } else {
        true
    };

    if needs_create {
        // we need a relative path from here
        let target = path(["..", VIDEO_DL_NAME, video_id]);
        info!("linking {} to {}", final_path.display(), target.display());
        std::os::unix::fs::symlink(&target, &final_path).map_err(SError::io(final_path))?;
    }

    Ok(())
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
