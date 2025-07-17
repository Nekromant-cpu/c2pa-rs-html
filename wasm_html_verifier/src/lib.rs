use wasm_bindgen::prelude::*;
use std::io::Cursor;
use web_sys::console;
use std::result::Result;
use c2pa::{assertions::Actions, Reader};

#[wasm_bindgen]
pub fn read_manifest(html: &str) -> Result<JsValue, JsValue> {
    let mut stream = Cursor::new(html.as_bytes());

    // Parse the manifest from the HTML stream
    let reader = Reader::from_stream("text/html", &mut stream)
        .map_err(|e| JsValue::from_str(&format!("Reader error: {}", e)))?;

    // Log manifest JSON to browser console
    let json_output = reader.json();
    console::log_1(&JsValue::from_str(&json_output));

    Ok(JsValue::from_str(&json_output))
}