use crate::err::SResult;
use scraper::{ElementRef, Html, Selector};
use simd_json::BorrowedValue;
use simd_json::base::ValueAsScalar;
use simd_json::derived::ValueObjectAccess;
use tracing::debug;

pub fn extract_original_id(content: &[u8]) -> SResult<String> {
    let document = Html::parse_document(str::from_utf8(&content).unwrap());

    let selector = Selector::parse("meta[name=one-data]").unwrap();
    let mut founds: Vec<ElementRef> = document.select(&selector).collect();
    if founds.len() != 1 {
        panic!("missing meta elem?")
    }
    let found = founds.remove(0);

    let mut one_config_raw: String = found.attr("data-one-config").unwrap().into();
    let one_config: BorrowedValue = unsafe { simd_json::from_str(&mut one_config_raw).unwrap() };

    // jq '.page.containerCollections[]  | .containers[] | .data.feed
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
