use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::Client;
use rquest_util::Emulation::Chrome136;
use rquest_util::EmulationOS::Windows;
use rquest_util::EmulationOption;
use std::time::Duration;

pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";
pub const CHROME_PLATFORM: &str = "\"Windows\"";
pub const CHROME_SEC_CH_UA: &str =
    "\"Not.A/Brand\";\"Chromium\";\"Google Chrome\";\"Chrome/136.0.0.0\"";
pub const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9";
pub const SEC_CH_UA_MOBILE: &str = "?0";
pub const ACCEPT_ENCODING: &str = "gzip, deflate, br";

#[derive(Clone)]
pub struct HttpClient {
    pub client: Client,
    pub referer: String,
    pub site_key: String,
}

impl HttpClient {
    pub fn new(referer: String, site_key: String) -> anyhow::Result<Self> {
        let client = build_chrome_client()?;
        Ok(Self {
            client,
            referer,
            site_key,
        })
    }
}

pub fn build_chrome_emulation() -> EmulationOption {
    EmulationOption::builder()
        .emulation(Chrome136)
        .emulation_os(Windows)
        .build()
}

pub fn build_chrome_client() -> anyhow::Result<Client> {
    let emulation = build_chrome_emulation();
    let header_map = build_default_headers();

    let client = Client::builder()
        .emulation(emulation)
        .gzip(true)
        .brotli(true)
        .deflate(false)
        .zstd(false)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .default_headers(header_map)
        .build()?;

    Ok(client)
}

pub fn build_default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        ),
    );
    headers.insert(
        HeaderName::from_static("accept-language"),
        HeaderValue::from_static(ACCEPT_LANGUAGE),
    );
    headers.insert(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_static(CHROME_UA),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua"),
        HeaderValue::from_static(CHROME_SEC_CH_UA),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua-mobile"),
        HeaderValue::from_static(SEC_CH_UA_MOBILE),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua-platform"),
        HeaderValue::from_static(CHROME_PLATFORM),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("document"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("navigate"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("none"),
    );
    headers.insert(
        HeaderName::from_static("upgrade-insecure-requests"),
        HeaderValue::from_static("1"),
    );
    headers.insert(
        HeaderName::from_static("accept-encoding"),
        HeaderValue::from_static(ACCEPT_ENCODING),
    );

    headers
}
