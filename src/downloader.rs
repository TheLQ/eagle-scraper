use crate::err::{SError, SResult};
use crate::global_config::GlobalConfig;
use reqwest::{Proxy, StatusCode};
use std::fs::{create_dir, read, write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use strum::{AsRefStr, VariantArray};
use tracing::{debug, info, trace, warn};

pub struct Downloader {
    client: reqwest::blocking::Client,
    last_request: Instant,
    config_domain: String,
}

#[derive(Clone, PartialEq, Eq, Hash, VariantArray, AsRefStr)]
pub enum DownType {
    HTML,
    Collection,
    Page,
}

pub struct FetchResponse {
    pub body: Vec<u8>,
    pub output_path: PathBuf,
}

pub const EXTRACTION_DB_ROOT: &str = "extraction-db";
const REQUEST_THROTTLE: Duration = Duration::from_secs(5); // Please be a nice scraper

impl Downloader {
    pub fn init(global_config: &GlobalConfig) -> Self {
        let proxy_addr = std::env::var("WARC_PROXY")
            .expect("Please be nice, export WARC_PROXY=127.0.0.1:8000 pointing to warcprox");

        Self {
            client: reqwest::blocking::Client::builder()
                // // proxy to MITM warcprox
                // .proxy(Proxy::all(format!("http://{proxy_addr}")).unwrap())
                // // which uses self-signed CA
                // .danger_accept_invalid_certs(true)
                // increase timeout. I think the proxy buffers the whole response first
                .timeout(Duration::from_mins(3))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:137.0) Gecko/20100101 Firefox/137.0")
                .build()
                .unwrap(),
            // arbitrary old date
            last_request: Instant::now() - Duration::from_days(1),
            config_domain: global_config.domain.clone(),
        }
    }

    pub fn fetch(&mut self, downtype: DownType, extra: &str) -> SResult<FetchResponse> {
        let config_domain = &self.config_domain;
        let safe_name: String;
        let url = match downtype {
            DownType::HTML => {
                assert_eq!(extra, "");
                safe_name = format!("page_home");
                format!("https://{config_domain}/")
            }
            DownType::Collection => {
                safe_name = format!("collection_{extra}");
                format!("https://{config_domain}/api/core/catalog/collection/{extra}")
            }
            DownType::Page => {
                safe_name = format!("frontend_{extra}");
                format!("https://{config_domain}/api/core/page/{extra}")
            }
        };
        let cache_path = path([EXTRACTION_DB_ROOT, &downtype.safe_name(), &safe_name]);
        if cache_path.exists() {
            debug!("cached url {url} at {}", cache_path.display());
            Ok(FetchResponse {
                body: read(&cache_path).map_err(SError::io(&cache_path))?,
                output_path: cache_path.into(),
            })
        } else {
            debug!("writing url {url} to {}", cache_path.display());

            let mut body = None;
            for i in 0..2 {
                if i != 0 {
                    warn!("retry {i}");
                }
                let throttle_safe: Instant = self.last_request + REQUEST_THROTTLE;
                let throttle_cur = Instant::now();
                let sleep_dur = throttle_safe - throttle_cur;
                if sleep_dur.as_secs() > 0 {
                    debug!("Throttle for {} secs", sleep_dur.as_secs());
                    thread::sleep(sleep_dur);
                }

                let request = self.client.get(&url).build()?;
                trace!("total headers {}", request.headers().len());
                for (name, value) in request.headers() {
                    trace!("HEADER {} - {}", name.to_string(), value.to_str().unwrap());
                }
                let response = self.client.execute(request)?;
                if response.status() != StatusCode::OK {
                    panic!("bad response {}", response.status());
                }
                body = Some(response.bytes()?);
                break;
            }
            let Some(body) = body else {
                panic!("failed to download {url}")
            };
            write(&cache_path, &body).map_err(SError::io(&cache_path))?;

            self.last_request = Instant::now();
            Ok(FetchResponse {
                body: body.to_vec(),
                output_path: cache_path.into(),
            })
        }
    }
}

impl DownType {
    pub fn mkdirs() {
        let mut output_dirs: Vec<PathBuf> = Self::VARIANTS
            .iter()
            .map(|downtype| path([EXTRACTION_DB_ROOT, &downtype.safe_name()]))
            .collect();
        output_dirs.insert(0, path([EXTRACTION_DB_ROOT]));

        for dir in output_dirs {
            if !dir.exists() {
                info!("Creating directory {}", dir.display());
                create_dir(&dir).unwrap();
            }
        }
    }

    fn safe_name(&self) -> String {
        self.as_ref().to_ascii_lowercase()
    }
}

pub fn path<const N: usize>(input: [&str; N]) -> PathBuf {
    input.into_iter().collect()
}
