use orc_sdk::{OrcHandle, deck, try_serialize_handle};
use server_host::server::{DECK_REGISTRY, OrcServer, next_handle_id};
use tinyjson::JsonValue;

fn start_server() -> (OrcServer, String) {
    let server = OrcServer::start(0).expect("Failed to start server");
    let base = format!("http://127.0.0.1:{}", server.port());
    (server, base)
}

fn post_json(url: &str, body: &str) -> (u16, JsonValue) {
    let resp = minreq::post(url)
        .with_header("Content-Type", "application/json")
        .with_body(body)
        .send()
        .expect("HTTP request failed");
    let code = resp.status_code as u16;
    let json: JsonValue = resp
        .as_str()
        .expect("Response not UTF-8")
        .parse()
        .expect("Response not valid JSON");
    (code, json)
}

fn post_bytes(url: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let resp = minreq::post(url)
        .with_header("Content-Type", "application/octet-stream")
        .with_body(body)
        .send()
        .expect("HTTP request failed");
    let code = resp.status_code as u16;
    (code, resp.into_bytes())
}

fn post_bytes_json(url: &str, body: &[u8]) -> (u16, JsonValue) {
    let resp = minreq::post(url)
        .with_header("Content-Type", "application/octet-stream")
        .with_body(body)
        .send()
        .expect("HTTP request failed");
    let code = resp.status_code as u16;
    let json: JsonValue = resp
        .as_str()
        .expect("Response not UTF-8")
        .parse()
        .expect("Response not valid JSON");
    (code, json)
}

fn get_json(url: &str) -> (u16, JsonValue) {
    let resp = minreq::get(url).send().expect("HTTP request failed");
    let code = resp.status_code as u16;
    let json: JsonValue = resp
        .as_str()
        .expect("Response not UTF-8")
        .parse()
        .expect("Response not valid JSON");
    (code, json)
}

fn json_u64(json: &JsonValue, key: &str) -> u64 {
    match json {
        JsonValue::Object(o) => match o.get(key) {
            Some(JsonValue::Number(n)) => *n as u64,
            other => panic!("Expected number for '{key}', got: {other:?}"),
        },
        _ => panic!("Expected JSON object"),
    }
}

fn json_arr<'a>(json: &'a JsonValue, key: &str) -> &'a Vec<JsonValue> {
    match json {
        JsonValue::Object(o) => match o.get(key) {
            Some(JsonValue::Array(arr)) => arr,
            other => panic!("Expected array for '{key}', got: {other:?}"),
        },
        _ => panic!("Expected JSON object"),
    }
}

fn json_nums(json: &JsonValue, key: &str) -> Vec<f64> {
    json_arr(json, key)
        .iter()
        .map(|v| match v {
            JsonValue::Number(n) => *n,
            _ => panic!("Expected number in array"),
        })
        .collect()
}

fn serialize_handle(handle: &OrcHandle) -> Vec<u8> {
    let mut buf = Vec::new();
    try_serialize_handle(handle, &mut buf).expect("Serialization failed");
    buf
}

#[test]
fn t_start_and_close_session() {
    let (_server, base) = start_server();
    let (code, json) = post_json(&format!("{base}/session/start"), "{}");
    assert_eq!(code, 200);
    let session_id = json_u64(&json, "session_id");
    assert!(session_id > 0);
    let (code, _) = post_json(
        &format!("{base}/session/close"),
        &format!(r#"{{"session_id": {session_id}}}"#),
    );
    assert_eq!(code, 200);
    let (code, _) = post_json(
        &format!("{base}/session/close"),
        &format!(r#"{{"session_id": {session_id}}}"#),
    );
    assert_eq!(code, 404);
}

#[test]
fn t_list_functions() {
    let (_server, base) = start_server();
    let (code, json) = get_json(&format!("{base}/functions"));
    assert_eq!(code, 200);
    let functions = json_arr(&json, "functions");
    assert!(!functions.is_empty(), "Should have loaded plugin functions");
    let has_add = functions.iter().any(|f| {
        if let JsonValue::Object(o) = f {
            matches!(o.get("name"), Some(JsonValue::String(s)) if s == "add")
        } else {
            false
        }
    });
    assert!(has_add, "Should have 'add' function");
}

#[test]
fn t_create_constant_and_download() {
    let (_server, base) = start_server();
    let (_, json) = post_json(&format!("{base}/session/start"), "{}");
    let sid = json_u64(&json, "session_id");
    // Serialize a deck via ABI and upload as constant.
    let d: orc_sdk::Deck<f64> = deck![1.0, 2.0, 3.0];
    let mut handle = OrcHandle {
        handle: next_handle_id(),
        ..Default::default()
    };
    DECK_REGISTRY
        .alloc_with_value(Some(d), &mut handle)
        .expect("Failed to allocate deck");
    let data = serialize_handle(&handle);
    handle.free();
    let (code, json) = post_bytes_json(&format!("{base}/constant?session_id={sid}"), &data);
    assert_eq!(code, 200);
    let hid = json_u64(&json, "handle_id");
    // Download the serialized handle.
    let (code, downloaded) = post_bytes(
        &format!("{base}/download?session_id={sid}&handle_id={hid}"),
        &[],
    );
    assert_eq!(code, 200);
    assert!(
        !downloaded.is_empty(),
        "Serialized data should not be empty"
    );
    // Upload the downloaded bytes as a new constant.
    let (code, json) = post_bytes_json(&format!("{base}/constant?session_id={sid}"), &downloaded);
    assert_eq!(code, 200);
    let hid2 = json_u64(&json, "handle_id");
    assert_ne!(hid, hid2);
    // Download again and verify same bytes.
    let (code, downloaded2) = post_bytes(
        &format!("{base}/download?session_id={sid}&handle_id={hid2}"),
        &[],
    );
    assert_eq!(code, 200);
    assert_eq!(downloaded, downloaded2, "Round-tripped data should match");
}

#[test]
fn t_call_add_function() {
    let (_server, base) = start_server();
    let (_, json) = post_json(&format!("{base}/session/start"), "{}");
    let sid = json_u64(&json, "session_id");
    // Create two constants.
    let d1: orc_sdk::Deck<f64> = deck![1.0, 2.0, 3.0];
    let mut h1 = OrcHandle {
        handle: next_handle_id(),
        ..Default::default()
    };
    DECK_REGISTRY.alloc_with_value(Some(d1), &mut h1).unwrap();
    let data1 = serialize_handle(&h1);
    h1.free();

    let d2: orc_sdk::Deck<f64> = deck![10.0, 20.0, 30.0];
    let mut h2 = OrcHandle {
        handle: next_handle_id(),
        ..Default::default()
    };
    DECK_REGISTRY.alloc_with_value(Some(d2), &mut h2).unwrap();
    let data2 = serialize_handle(&h2);
    h2.free();

    let (_, json) = post_bytes_json(&format!("{base}/constant?session_id={sid}"), &data1);
    let a_id = json_u64(&json, "handle_id");
    let (_, json) = post_bytes_json(&format!("{base}/constant?session_id={sid}"), &data2);
    let b_id = json_u64(&json, "handle_id");
    // Call add(a, b).
    let (code, json) = post_json(
        &format!("{base}/call"),
        &format!(r#"{{"session_id": {sid}, "function": "add", "inputs": [{a_id}, {b_id}]}}"#),
    );
    assert_eq!(code, 200);
    let out_ids = json_nums(&json, "output_ids");
    assert_eq!(out_ids.len(), 1);
    let out_id = out_ids[0] as u64;
    // Download result.
    let (code, data) = post_bytes(
        &format!("{base}/download?session_id={sid}&handle_id={out_id}"),
        &[],
    );
    assert_eq!(code, 200);
    assert!(!data.is_empty());
}
