use crate::err::SResult;
use scraper::{ElementRef, Html, Selector};
use simd_json::BorrowedValue;
use simd_json::base::{ValueAsArray, ValueAsScalar};
use simd_json::derived::ValueObjectAccess;
use tracing::debug;

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
    let mut container_collections = json
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

        if feed.contains("watch-history") {
            debug!("skipping watch {feed}")
        } else if let Some((original, limit)) = feed.split_once("?") {
            debug!("found feed {original} stripped ?{limit} ");
            feeds.push(original.into());
        } else {
            debug!("found feed {feed}");
            feeds.push(feed.into());
        }
    }

    Ok(feeds)
}
