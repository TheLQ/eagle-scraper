use crate::err::SResult;
use crate::utils::last_position_of;
use scraper::{ElementRef, Html, Selector};
use simd_json::BorrowedValue;
use simd_json::prelude::{ValueObjectAccess, ValueObjectAccessAsArray, ValueObjectAccessAsScalar};
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
        .get_str("HOME")
        .expect("HOME");
    debug!("extracted ROOT URL {root_id}");

    Ok(root_id.into())
}

pub fn extract_collections_from_root(mut content: Vec<u8>) -> SResult<Vec<ExtractedThing>> {
    let json: BorrowedValue = simd_json::to_borrowed_value(&mut content).unwrap();

    // jq '.page.containerCollections[]  | .containers[] | .data.feed'
    let container_collections = json
        .get("page")
        .expect("pages")
        .get_array("containerCollections")
        .expect("containerCollections");
    assert_eq!(container_collections.len(), 1, "containerCollections len");
    let container_collection = &container_collections[0];

    let containers = container_collection
        .get_array("containers")
        .expect("containers");
    let mut feeds = Vec::new();
    for container in containers {
        let feed = container
            .get("data")
            .expect("data")
            .get_str("feed")
            .expect("feed");

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
        feeds.push(ExtractedThing {
            next_id: collection_id.into(),
            next_type: ThingType::Collection,
            title: "collection from page".into(),
        });
    }

    Ok(feeds)
}

pub fn extract_things_from_collection(mut content: Vec<u8>) -> SResult<Vec<ExtractedThing>> {
    let json: BorrowedValue = simd_json::to_borrowed_value(&mut content).unwrap();

    let has_next = json
        .get("pageInfo")
        .expect("pageInfo")
        .get_bool("hasMore")
        .expect("hasMore");
    if has_next {
        todo!("previously no multi page collections");
    }

    // jq '.data[] | select(.subtype | contains("VIDEO")) | .id'
    // jq '.data[] | select(.subtype | contains("VIDEO")) | .video.playback' (alt)
    let data_arr = json.get_array("data").expect("data");
    let mut video_ids = Vec::new();
    for item in data_arr {
        let title = item.get_str("title").expect("title");
        if let Some(actions) = item.get_array("actions") {
            assert_eq!(actions.len(), 1);
            let action = &actions[0];
            assert_eq!(action.get_str("kind").expect("kind"), "NAVIGATE_TO_PAGE");
            // both params and parameters? idk pick one
            let id = action
                .get("params")
                .expect("params")
                .get_str("id")
                .expect("id");
            video_ids.push(ExtractedThing {
                title: title.into(),
                next_id: id.into(),
                next_type: ThingType::Page,
            })
        }

        let id = match item.get_str("subtype").expect("subtype") {
            "GENERIC" => {
                trace!("skipping generic");
                continue;
            }
            "VIDEO" => item.get_str("id").expect("id"),
            unknown => panic!("unknown id {unknown}"),
        };
        trace!("found video {id}");
        video_ids.push(ExtractedThing {
            title: title.into(),
            next_id: id.into(),
            next_type: ThingType::Video,
        });
    }

    Ok(video_ids)
}

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub struct ExtractedThing {
    pub title: String,
    pub next_type: ThingType,
    pub next_id: String,
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum ThingType {
    Video,
    Collection,
    Page,
}
