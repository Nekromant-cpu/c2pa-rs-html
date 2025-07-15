use wasm_bindgen::prelude::*;
use std::io::Cursor;
use web_sys::console;

// use c2pa::{
//     CAIRead, HtmlIO,
//     asset_io::CAIReader,
//     verifier::Verifier,
//     store::Store,
//     Error,
// };

// #[wasm_bindgen]
// pub fn verify_html_manifest(html: &str) -> bool {
//     let mut cursor = Cursor::new(html.as_bytes());
//     let html_io = HtmlIO {};

//     let store = Store::new();

//     // Construct the Verifier with the store
//     let verifier = Verifier::new(store);

//     // Attempt to verify using the HtmlIO implementation
//     match verifier.verify_from_asset_io(&html_io, &mut cursor) {
//         Ok(_) => true,
//         Err(_) => false,
//     }
// }

// #[wasm_bindgen]
// pub fn has_manifest(html: &str) -> bool {
//     let mut cursor = Cursor::new(html.as_bytes());
//     let html_io = HtmlIO {};

//     html_io.read_cai(&mut cursor).is_ok()
// }

// #[wasm_bindgen]
// pub fn get_html_manifest(html: &str) -> Result<String, JsValue> {
//     let mut cursor = Cursor::new(html.as_bytes());
//     let html_io = HtmlIO {};

//     match html_io.read_cai(&mut cursor) {
//         Ok(manifest_b64_string) => Ok(manifest_b64_string),
//         Err(e) => Err(JsValue::from_str(&format!("Failed to extract manifest: {}", e))),
//     }
// }


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

    // Collect action labels if available
    if let Some(manifest) = reader.active_manifest() {
        if let Ok(actions) = manifest.find_assertion::<Actions>(Actions::LABEL) {
            let action_list: Vec<String> = actions.actions
                .iter()
                .map(|action| action.action().to_string())
                .collect();
            let json = serde_json::to_string(&action_list)
                .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))?;

            return Ok(JsValue::from_str(&json));
        }
    }

    Ok(JsValue::from_str("No actions found"))
}