use std::io::{BufRead};

use crate::{
    asset_io::{AssetIO, CAIRead, CAIReadWrite, CAIReader, CAIWriter, HashBlockObjectType, HashObjectPositions},
    error::{Error, Result},
};
use std::{
    fs::{self, File},
    io::{Read, Write, SeekFrom, BufReader},
    path::{Path, PathBuf},
};

static SUPPORTED_TYPES: [&str; 2] = [
    "html",
    "text/html"
];

const C2PA_LINK_REL: &str = "c2pa-manifest";
const C2PA_LINK_TYPE: &str = "application/c2pa-manifest+json";

fn sidecar_path(asset_path: &Path) -> PathBuf {
    let mut p = asset_path.to_path_buf();
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    p.set_extension(format!("{ext}.c2pa"));
    p
}

fn manifest_link_tag(asset_path: &Path) -> String {
    let binding = sidecar_path(asset_path);
    let filename = binding
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("manifest.c2pa");
    format!(
        r#"<link rel="{rel}" href="{href}" type="{typ}"/>"#,
        rel = C2PA_LINK_REL,
        href = filename,
        typ = C2PA_LINK_TYPE
    )
}

/// Inserts or updates the manifest <link> tag in the HTML file.
fn insert_or_update_manifest_link(asset_path: &Path) -> Result<()> {
    let html_path = asset_path;
    let link_tag = manifest_link_tag(asset_path);

    let mut html = String::new();
    {
        let mut f = File::open(html_path)?;
        f.read_to_string(&mut html)?;
    }

    let mut new_html = String::new();
    let mut link_inserted = false;
    let mut in_head = false;

    for line in html.lines() {
        // Insert or update the link tag inside <head>
        if line.trim_start().starts_with("<head") {
            in_head = true;
            new_html.push_str(line);
            new_html.push('\n');
            continue;
        }
        if in_head && line.trim_start().starts_with("</head>") {
            if !link_inserted {
                new_html.push_str("    ");
                new_html.push_str(&link_tag);
                new_html.push('\n');
                link_inserted = true;
            }
            in_head = false;
        }
        // Remove any existing c2pa-manifest link
        if line.contains(r#"rel="c2pa-manifest""#) {
            if !link_inserted {
                new_html.push_str("    ");
                new_html.push_str(&link_tag);
                new_html.push('\n');
                link_inserted = true;
            }
            continue;
        }
        new_html.push_str(line);
        new_html.push('\n');
    }

    // If no <head> found, prepend the link at the top (not ideal, but fallback)
    if !link_inserted {
        new_html = format!("{}\n{}", link_tag, new_html);
    }

    let mut f = File::create(html_path)?;
    f.write_all(new_html.as_bytes())?;
    Ok(())
}

/// Finds the manifest sidecar path by parsing the HTML file for the <link rel="c2pa-manifest"> tag.
/// Uses the directory of the input file if possible.
fn find_manifest_sidecar_from_html(input_stream: &mut dyn CAIRead) -> Option<PathBuf> {

    let _ = input_stream.rewind();

    let mut reader = BufReader::new(input_stream);
    let mut line = String::new();
    while reader.read_line(&mut line).ok()? > 0 {
        if let Some(idx) = line.find(r#"rel="c2pa-manifest""#) {
            // Try to extract href="...".
            if let Some(href_start) = line[idx..].find(r#"href=""#) {
                let rest = &line[idx + href_start + 6..];
                if let Some(href_end) = rest.find('"') {
                    let href = &rest[..href_end];
                    // Try to get the directory of the underlying file if possible.
                    // If not, just use the href as a relative path.
                    // if let Some(file) = reader.get_ref().downcast_ref::<File>() {
                    //     if let Some(parent) = file.path().parent() {
                    //         return Some(parent.join(href));
                    //     }
                    // }
                    return Some(PathBuf::from(href));
                }
            }
        }
        line.clear();
    }
    None
}

pub struct HtmlIO {}

impl CAIReader for HtmlIO {
    fn read_cai(&self, asset_reader: &mut dyn CAIRead) -> Result<Vec<u8>> {
        // Try to find the manifest sidecar via the <link> tag in the HTML
        if let Some(sidecar) = find_manifest_sidecar_from_html(asset_reader) {
            let mut f = File::open(&sidecar).map_err(|_| Error::JumbfNotFound)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            return Ok(buf);
        }
        Err(Error::JumbfNotFound)
    }

    fn read_xmp(&self, _reader: &mut dyn CAIRead) -> Option<String> {
        // Not used for stream reading; see AssetIO::read_xmp_store
        None
    }
}

impl CAIWriter for HtmlIO {
    fn write_cai(
        &self,
        _asset_reader: &mut dyn CAIRead,
        asset_writer: &mut dyn CAIReadWrite,
        store_bytes: &[u8],
    ) -> Result<()> {
        asset_writer.write_all(store_bytes)?;
        Ok(())
    }

    fn get_object_locations_from_stream(
        &self,
        input_stream: &mut dyn CAIRead,
    ) -> Result<Vec<HashObjectPositions>> {
        let len = input_stream.seek(SeekFrom::End(0))? as usize;
        input_stream.rewind()?;
        Ok(vec![HashObjectPositions {
            offset: 0,
            length: len,
            htype: HashBlockObjectType::Cai,
        }])
    }

    fn remove_cai_store_from_stream(
        &self,
        _asset_reader: &mut dyn CAIRead,
        asset_writer: &mut dyn CAIReadWrite,
    ) -> Result<()> {
        asset_writer.rewind()?;
        Ok(())
    }
}

impl AssetIO for HtmlIO {
    fn new(_asset_type: &str) -> Self
    where
        Self: Sized,
    {
        HtmlIO {}
    }

    fn get_handler(&self, asset_type: &str) -> Box<dyn AssetIO> {
        Box::new(HtmlIO::new(asset_type))
    }

    fn get_reader(&self) -> &dyn CAIReader {
        self
    }

    fn get_writer(&self, asset_type: &str) -> Option<Box<dyn CAIWriter>> {
        Some(Box::new(HtmlIO::new(asset_type)))
    }

    fn read_cai_store(&self, asset_path: &Path) -> Result<Vec<u8>> {
        // Use the <link> tag to find the manifest sidecar
        let mut f = File::open(asset_path)?;

        self.read_cai(&mut f)
    }

    fn save_cai_store(&self, asset_path: &Path, store_bytes: &[u8]) -> Result<()> {
        let sidecar = sidecar_path(asset_path);
        let mut f = File::create(&sidecar)?;
        f.write_all(store_bytes)?;
        // Insert or update the manifest link in the HTML file
        insert_or_update_manifest_link(asset_path)?;
        Ok(())
    }

    fn get_object_locations(&self, asset_path: &Path) -> Result<Vec<HashObjectPositions>> {
        let mut input_file = File::open(asset_path)?;
        if let Some(sidecar) = find_manifest_sidecar_from_html(&mut input_file) {
            let len = std::fs::metadata(&sidecar)
                .map_err(|_| Error::JumbfNotFound)?
                .len() as usize;
            Ok(vec![HashObjectPositions {
                offset: 0,
                length: len,
                htype: HashBlockObjectType::Cai,
            }])
        } else {
            Err(Error::JumbfNotFound)
        }
    }

    fn remove_cai_store(&self, asset_path: &Path) -> Result<()> {
        let mut input_file = File::open(asset_path)?;

        if let Some(sidecar) = find_manifest_sidecar_from_html(&mut input_file) {
            if sidecar.exists() {
                fs::remove_file(sidecar)?;
            }
        }
        Ok(())
    }

    fn supported_types(&self) -> &[&str] {
        &SUPPORTED_TYPES
    }
}