use crate::{
    asset_io::{rename_or_move, AssetIO, CAIRead, CAIReadWrite, CAIReader, CAIWriter,
        HashObjectPositions, HashBlockObjectType},
    error::{Error, Result},
    utils::{
        io_utils::{tempfile_builder},
    },
};
use std::{
    fs::{File},
    path::{Path},
};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use regex::Regex;

static SUPPORTED_TYPES: [&str; 2] = [
    "html",
    "text/html"
];

const C2PA_SCRIPT_TYPE: &str = "application/c2pa-manifest";

// Regex patterns
/// <script type="application/c2pa-manifest">BASE64_ENCODED_MANIFEST</script>
const C2PA_REGEX_CAPTURE: &str = r#"(?s)<script[^>]*type=["']application/c2pa-manifest["'][^>]*>(.*?)</script>"#;
const C2PA_REGEX_FULL: &str = r#"(?s)\s*<script[^>]*type=["']application/c2pa-manifest["'][^>]*>.*?</script>\s*"#;
const HTML_HEAD_TAG: &str = r#"(?i)<head[^>]*>"#;

static DEBUG: bool = false; // Set to true to enable debug prints

pub struct HtmlIO {}


impl CAIReader for HtmlIO {
    
    /// read manifest from embedded data
    fn read_cai(&self, asset_reader: &mut dyn CAIRead) -> Result<Vec<u8>> {
        if DEBUG { println!("read_cai"); }
        
        let (manifest_opt, _insertion_point) = detect_manifest_location(asset_reader)?;

        match manifest_opt {
            Some(data) if !data.is_empty() => Ok(data),
            _ => Err(Error::JumbfNotFound),
        }
    }

    fn read_xmp(&self, _reader: &mut dyn CAIRead) -> Option<String> {
        if DEBUG { println!("read_xmp"); }
        None
    }
}

impl CAIWriter for HtmlIO {

    /// embed manifest into HTML
    fn write_cai(
        &self,
        input_stream: &mut dyn CAIRead,
        output_stream: &mut dyn CAIReadWrite,
        store_bytes: &[u8],
    ) -> Result<()> {
        if DEBUG { println!("write_cai"); }

        let mut input_html = String::new();
        input_stream.rewind()?;
        input_stream.read_to_string(&mut input_html)?;

        let manifest_b64 = STANDARD.encode(store_bytes);
        let manifest_script = format!(r#"<script type="{C2PA_SCRIPT_TYPE}">{manifest_b64}</script>"#);

        // Regex to match optional whitespace before </body>
        let re_body = Regex::new(r"(?i)\s*</body>").unwrap();

        let re = regex::Regex::new(C2PA_REGEX_FULL)
            .map_err(|_| Error::InvalidAsset("Regex error".into()))?;

        let updated_html = if re.is_match(&input_html) {
            re.replace(&input_html, &manifest_script).into_owned()
        } else if re_body.is_match(&input_html) {
            // Case 2: Insert before </body>, removing leading whitespace
            re_body
                .replace(&input_html, format!("{manifest_script}</body>"))
                .into_owned()
        } else {
            let trimmed = input_html.trim_end();
            format!("{}{}", trimmed, manifest_script)
        };

        output_stream.rewind()?;
        output_stream.write_all(updated_html.as_bytes())?;
        Ok(())
    }

    /// locate the position of the embedded manifest to exclude it from hashing
    fn get_object_locations_from_stream(
        &self,
        input_stream: &mut dyn CAIRead,
    ) -> Result<Vec<HashObjectPositions>> {
        if DEBUG { println!("get_object_locations_from_stream"); }
        
        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut output_stream = std::io::Cursor::new(&mut buffer);
            add_required_segs_to_stream(input_stream, &mut output_stream)?;
        }

        let mut buffer_cursor = std::io::Cursor::new(&buffer);
        let (manifest_opt, insertion_point) =
            detect_manifest_location(&mut buffer_cursor)?;

        let manifest = manifest_opt.ok_or(Error::JumbfNotFound)?;
        let b64_len = STANDARD.encode(&manifest).len();
        let start = insertion_point;
        let html_len = buffer.len();

        Ok(vec![
            HashObjectPositions {
                offset: start,
                length: b64_len,
                htype: HashBlockObjectType::Cai, // this will be excluded from hashing
            },
            HashObjectPositions {
                offset: 0,
                length: start,
                htype: HashBlockObjectType::Other,
            },
            HashObjectPositions {
                offset: start + b64_len,
                length: html_len.saturating_sub(start + b64_len),
                htype: HashBlockObjectType::Other,
            },
        ])
    }

    /// remove the manifest from the html file stream
    fn remove_cai_store_from_stream(
        &self,
        input_stream: &mut dyn CAIRead,
        output_stream: &mut dyn CAIReadWrite,
    ) -> Result<()> {
        if DEBUG { println!("remove_cai_store_from_stream"); }

        let mut html = String::new();
        input_stream.read_to_string(&mut html)?;

        let re = regex::Regex::new(C2PA_REGEX_FULL)
            .map_err(|_| Error::InvalidAsset("Regex error".into()))?;

        let cleaned = re.replace(&html, "").into_owned();

        output_stream.rewind()?;
        output_stream.write_all(cleaned.as_bytes())?;
        Ok(())
    }
}

impl AssetIO for HtmlIO {
    fn new(_asset_type: &str) -> Self {
        if DEBUG { println!("new"); }

        HtmlIO {}
    }

    fn get_handler(&self, asset_type: &str) -> Box<dyn AssetIO> {
        if DEBUG { println!("get_handler"); }

        Box::new(HtmlIO::new(asset_type))
    }

    fn get_reader(&self) -> &dyn CAIReader {
        if DEBUG { println!("get_reader"); }

        self
    }

    fn get_writer(&self, _asset_type: &str) -> Option<Box<dyn CAIWriter>> {
        if DEBUG { println!("get_writer"); }

        Some(Box::new(HtmlIO {}))
    }

    fn read_cai_store(&self, asset_path: &Path) -> Result<Vec<u8>> {
        if DEBUG { println!("read_cai_store: {}", asset_path.display()); }
        
        let mut f = File::open(asset_path)?;
        self.read_cai(&mut f)
    }

    fn save_cai_store(&self, asset_path: &Path, store_bytes: &[u8]) -> Result<()> {
        if DEBUG { println!("save_cai_store: {}", asset_path.display()); }
        
        let mut input_stream = std::fs::OpenOptions::new()
            .read(true)
            .open(asset_path)
            .map_err(Error::IoError)?;
        let mut temp_file = tempfile_builder("c2pa_temp")?;
        self.write_cai(&mut input_stream, &mut temp_file, store_bytes)?;
        rename_or_move(temp_file, asset_path)
    }

    fn get_object_locations(&self, asset_path: &Path) -> Result<Vec<HashObjectPositions>> {
        if DEBUG { println!("get_object_locations: {}", asset_path.display()); }
        
        let mut input_stream = std::fs::File::open(asset_path).map_err(|_err| Error::EmbeddingError)?;
        self.get_object_locations_from_stream(&mut input_stream)
    }

    fn remove_cai_store(&self, asset_path: &Path) -> Result<()> {
        if DEBUG { println!("remove_cai_store: {}", asset_path.display()); }
        
        let mut input_file = File::open(asset_path)?;
        let mut temp_file = tempfile_builder("c2pa_temp")?;
        self.remove_cai_store_from_stream(&mut input_file, &mut temp_file)?;
        rename_or_move(temp_file, asset_path)
    }

    fn supported_types(&self) -> &[&str] {
        if DEBUG { println!("supported_types"); }

        &SUPPORTED_TYPES
    }
}

/// prepare the html stream by including a dummy manifest if no manifest is present
fn add_required_segs_to_stream(
    input_stream: &mut dyn CAIRead,
    output_stream: &mut dyn CAIReadWrite,
) -> Result<()> {
    if DEBUG { println!("add_required_segs_to_stream"); }

    let (encoded_manifest_opt, _insertion_point) =
        detect_manifest_location(input_stream)?;

    let need_manifest = if let Some(encoded_manifest) = encoded_manifest_opt {
        encoded_manifest.is_empty()
    } else {
        true
    };

    if need_manifest {
        // Placeholder manifest to be inserted into HTML
        let data: &str = "placeholder manifest";

        let html = HtmlIO::new("html");
        let html_writer = html.get_writer("html").ok_or(Error::UnsupportedType)?;

        html_writer.write_cai(input_stream, output_stream, data.as_bytes())?;
    } else {
        // Just clone the input to the output
        input_stream.rewind()?;
        output_stream.rewind()?;
        std::io::copy(input_stream, output_stream)?;
    }

    Ok(())
}

/// find the location of the manifest inside the html stream
/// returns the manifest_opt and the location
fn detect_manifest_location(
    input_stream: &mut dyn CAIRead,
) -> Result<(Option<Vec<u8>>, usize)> {
    if DEBUG { println!("detect_manifest_location"); }

    input_stream.rewind()?;

    let mut html = String::new();
    input_stream.read_to_string(&mut html)?;

    let mut output: Option<Vec<u8>> = None;
    let mut insertion_point: usize = 0;

    // 1. Try to capture existing manifest content
    let manifest_re = Regex::new(C2PA_REGEX_CAPTURE).unwrap();
    if let Some(caps) = manifest_re.captures(&html) {
        if let Some(encoded) = caps.get(1) {
            let trimmed = encoded.as_str().trim();
            if !trimmed.is_empty() {
                output = Some(STANDARD.decode(trimmed).map_err(|_| {
                    Error::InvalidAsset("HTML manifest bad base64 encoding".into())
                })?);
                insertion_point = encoded.start(); //insertion_point = caps.get(0).unwrap().start(); // Position of the full tag
            }
        }
    }

    // 2. If no manifest found, try to locate <head> tag for insertion
    if output.is_none() {
        if DEBUG { println!("no manifest found"); }
        let head_re = Regex::new(HTML_HEAD_TAG).unwrap();
        if let Some(head_match) = head_re.find(&html) {
            insertion_point = head_match.end(); // Right after the <head> tag
        }
    }

    Ok((output, insertion_point))
}