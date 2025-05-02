use crate::err::{SError, SResult};
use std::collections::HashMap;
use std::path::Path;

pub struct GlobalConfig {
    pub domain: String,
    pub bc_account_id: String,
}

impl GlobalConfig {
    pub fn load() -> SResult<Self> {
        let path = Path::new(".env");
        let raw = std::fs::read(&path).map_err(SError::io(path))?;

        let lines = String::from_utf8(raw).unwrap();
        let mut config_map: HashMap<&str, &str> = lines
            .lines()
            .map(|line| line.split_once("=").unwrap())
            // .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let config = Self {
            domain: config_map.remove("DOMAIN").unwrap().into(),
            bc_account_id: config_map.remove("BC_ACCOUNT_ID").unwrap().into(),
        };
        Ok(config)
    }
}
