use crate::err::SResult;
use crate::utils::last_position_of;
use scraper::{ElementRef, Html, Selector};
use simd_json::BorrowedValue;
use simd_json::base::{ValueAsArray, ValueAsScalar};
use simd_json::derived::ValueObjectAccess;
use tracing::{debug, trace};

pub fn extract_original_id(content: &[u8]) -> SResult<String> {
    let document = Html::parse_document(str::from_utf8(content).unwrap());

    let selector = Selector::parse("meta[name=one-data]").unwrap();
    let mut founds: Vec<ElementRef> = document.select(&selector).collect();
    if founds.len() != 1 {
        panic!("missing meta elem?")
    }
    let found = founds.remove(0);

    let mut one_config_raw: String = found.attr("data-one-config").unwrap().into();
    let one_config: BorrowedValue = unsafe { simd_json::from_str(&mut one_config_raw).unwrap() };

    let root_id = one_config
        .get("pages")
        .expect("pages")
        .get("HOME")
        .expect("HOME")
        .as_str()
        .expect("HOME str");
    debug!("extracted ROOT URL {root_id}");

    Ok(root_id.into())
}

pub fn extract_collections_from_root(mut content: Vec<u8>) -> SResult<Vec<String>> {
    let json: BorrowedValue = simd_json::to_borrowed_value(&mut content).unwrap();

    // jq '.page.containerCollections[]  | .containers[] | .data.feed'
    let container_collections = json
        .get("page")
        .expect("pages")
        .get("containerCollections")
        .expect("containerCollections")
        .as_array()
        .expect("containerCollections array");
    assert_eq!(container_collections.len(), 1, "containerCollections len");
    let container_collection = &container_collections[0];

    let containers = container_collection
        .get("containers")
        .expect("containers")
        .as_array()
        .expect("containers array");
    let mut feeds = Vec::new();
    for container in containers {
        let feed = container
            .get("data")
            .expect("data")
            .get("feed")
            .expect("feed")
            .as_str()
            .expect("feed str");

        let feed_url = if feed.contains("watch-history") {
            debug!("skipping watch {feed}");
            continue;
        } else if let Some((original, limit)) = feed.split_once("?") {
            debug!("found feed {original} stripped ?{limit} ");
            original
        } else {
            debug!("found feed {feed}");
            feed
        };
        let collection_id = &feed_url[(last_position_of(feed_url, b'/') + 1)..];
        trace!("id {collection_id}");
        feeds.push(collection_id.into());
    }

    Ok(feeds)
}

pub fn extract_video_from_collection(mut content: Vec<u8>) -> SResult<Vec<String>> {
    let json: BorrowedValue = simd_json::to_borrowed_value(&mut content).unwrap();

    // jq '.data[] | select(.subtype | contains("VIDEO")) | .id'
    // jq '.data[] | select(.subtype | contains("VIDEO")) | .video.playback' (alt)
    let data_arr = json
        .get("data")
        .expect("data")
        .as_array()
        .expect("data array");
    let mut video_ids = Vec::new();
    for item in data_arr {
        let subtype = item
            .get("subtype")
            .expect("subtype")
            .as_str()
            .expect("subtype str");
        let id = match subtype {
            "GENERIC" => {
                trace!("skipping generic");
                continue;
            }
            "VIDEO" => item.get("id").expect("id").as_str().expect("id str"),
            unknown => panic!("unknown id {unknown}"),
        };
        trace!("found video {id}");
        video_ids.push(id.into());
    }

    Ok(video_ids)
}
