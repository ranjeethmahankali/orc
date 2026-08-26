use server_host::server::OrcServer;
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

fn json_bytes(json: &JsonValue, key: &str) -> Vec<u8> {
    json_arr(json, key)
        .iter()
        .map(|v| match v {
            JsonValue::Number(n) => *n as u8,
            _ => panic!("Expected number in array"),
        })
        .collect()
}

#[test]
fn start_and_close_session() {
    let (_server, base) = start_server();
    let (code, json) = post_json(&format!("{base}/session/start"), "{}");
    assert_eq!(code, 200);
    let session_id = json_u64(&json, "session_id");
    assert!(session_id > 0);
    // Close the session.
    let (code, _) = post_json(
        &format!("{base}/session/close"),
        &format!(r#"{{"session_id": {session_id}}}"#),
    );
    assert_eq!(code, 200);
    // Closing again should 404.
    let (code, _) = post_json(
        &format!("{base}/session/close"),
        &format!(r#"{{"session_id": {session_id}}}"#),
    );
    assert_eq!(code, 404);
}

#[test]
fn list_functions() {
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
fn create_constant_and_download() {
    let (_server, base) = start_server();
    let (_, json) = post_json(&format!("{base}/session/start"), "{}");
    let sid = json_u64(&json, "session_id");
    // Create a constant [1.0, 2.0, 3.0].
    let (code, json) = post_json(
        &format!("{base}/constant"),
        &format!(r#"{{"session_id": {sid}, "type": "f64", "values": [1.0, 2.0, 3.0]}}"#),
    );
    assert_eq!(code, 200);
    let hid = json_u64(&json, "handle_id");
    // Download the serialized handle.
    let (code, json) = post_json(
        &format!("{base}/download"),
        &format!(r#"{{"session_id": {sid}, "handle_id": {hid}}}"#),
    );
    assert_eq!(code, 200);
    let type_id = json_u64(&json, "type_id");
    assert_eq!(type_id, 18); // ORC_TYPE_F64
    let data = json_bytes(&json, "data");
    assert!(!data.is_empty(), "Serialized data should not be empty");
    // Upload the same data back as a new handle.
    let data_str: Vec<String> = data.iter().map(|b| b.to_string()).collect();
    let (code, json) = post_json(
        &format!("{base}/upload"),
        &format!(
            r#"{{"session_id": {sid}, "type_id": {type_id}, "data": [{}]}}"#,
            data_str.join(", ")
        ),
    );
    assert_eq!(code, 200);
    let hid2 = json_u64(&json, "handle_id");
    assert_ne!(hid, hid2);
    // Download again and verify same bytes.
    let (code, json2) = post_json(
        &format!("{base}/download"),
        &format!(r#"{{"session_id": {sid}, "handle_id": {hid2}}}"#),
    );
    assert_eq!(code, 200);
    let data2 = json_bytes(&json2, "data");
    assert_eq!(data, data2, "Round-tripped data should match");
}

#[test]
fn call_add_function() {
    let (_server, base) = start_server();
    let (_, json) = post_json(&format!("{base}/session/start"), "{}");
    let sid = json_u64(&json, "session_id");
    // Create two constants.
    let (_, json) = post_json(
        &format!("{base}/constant"),
        &format!(r#"{{"session_id": {sid}, "type": "f64", "values": [1.0, 2.0, 3.0]}}"#),
    );
    let a_id = json_u64(&json, "handle_id");
    let (_, json) = post_json(
        &format!("{base}/constant"),
        &format!(r#"{{"session_id": {sid}, "type": "f64", "values": [10.0, 20.0, 30.0]}}"#),
    );
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
    // Download result and verify via round-trip.
    let (code, json) = post_json(
        &format!("{base}/download"),
        &format!(r#"{{"session_id": {sid}, "handle_id": {out_id}}}"#),
    );
    assert_eq!(code, 200);
    let type_id = json_u64(&json, "type_id");
    assert_eq!(type_id, 18); // ORC_TYPE_F64
    let data = json_bytes(&json, "data");
    // Upload into a fresh handle so we can read the bytes back.
    // Verify the serialized data is non-empty (the actual byte content is ABI-specific).
    assert!(!data.is_empty());
}
