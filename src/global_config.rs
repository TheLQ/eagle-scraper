use crate::err::{SError, SResult};
use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub struct GlobalConfig {
    pub domain: String,
    pub bc_account_id: String,
    pub missing_videos: Vec<String>,
}

impl GlobalConfig {
    pub fn load() -> SResult<Self> {
        let path = Path::new(".env");
        let raw = std::fs::read(path).map_err(SError::io(path))?;

        let lines_raw = String::from_utf8(raw).unwrap();

        let mut config_map = HashMap::new();
        let mut missing_videos = Vec::new();
        for line in lines_raw.lines() {
            if line.starts_with("#") {
                continue;
            }
            let (k, v) = line.split_once("=").unwrap();
            if k == "missing" {
                missing_videos.push(v.to_string());
            } else {
                config_map.insert(k, v);
            }
        }

        let config = Self {
            domain: config_map.remove("DOMAIN").unwrap().into(),
            bc_account_id: config_map.remove("BC_ACCOUNT_ID").unwrap().into(),
            missing_videos,
        };
        Ok(config)
    }
}
