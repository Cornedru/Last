use anyhow::Result;
use flate2::read::GzDecoder;
use std::io::Read;

pub fn decompress_body(bytes: &[u8], encoding: &str) -> Result<Vec<u8>> {
    match encoding.to_lowercase().as_str() {
        "gzip" => {
            let mut decoder = GzDecoder::new(bytes);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(decompressed)
        }
        "br" => {
            let mut decompressed = Vec::new();
            let mut input = bytes;
            brotli::BrotliDecompress(&mut input, &mut decompressed)?;
            Ok(decompressed)
        }
        "" | "identity" => Ok(bytes.to_vec()),
        _ => Ok(bytes.to_vec()),
    }
}

pub fn extract_c_ray(html: &str) -> Option<String> {
    let pattern = r#"data-ray="([^"]+)""#;
    let re = regex::Regex::new(pattern).ok()?;
    let caps = re.captures(html)?;
    caps.get(1).map(|m| m.as_str().to_string())
}
