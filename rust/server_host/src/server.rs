use orc_sdk::{
    ContextArena, Deck, DeckRegistry, DeckView, Error, ORC_ABI_VERSION, ORC_DECK_PROXY_COPY_ALL,
    ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE, ORC_ERROR_INVALID_PROXY, ORC_ERROR_NONE,
    ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8,
    ORC_TYPE_U16, ORC_TYPE_U32, ORC_TYPE_U64, OrcError, OrcHandle, OrcHandleBorrowed, OrcHost,
    OrcHostCallbackAPI, OrcHostMemoryAPI, OrcProxyType, PluginSet, ProxyType, TOrcData,
    reset_handle, slice_from_ptr,
};
use std::{
    alloc::{Layout, alloc, dealloc},
    any::Any,
    collections::HashMap,
    ffi::{CStr, c_void},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::JoinHandle,
};
use tiny_http::{Method, Request, Response, Server, StatusCode};
use tinyjson::JsonValue;

// --- Host callbacks (same pattern as kbb_cli_host) ---

unsafe extern "C" fn host_alloc(size: u64, alignment: u64) -> *mut c_void {
    let layout = Layout::from_size_align(size as usize, alignment as usize).unwrap();
    unsafe { alloc(layout) as *mut c_void }
}

unsafe extern "C" fn host_dealloc(ptr: *mut c_void, size: u64, alignment: u64) {
    let layout = Layout::from_size_align(size as usize, alignment as usize).unwrap();
    unsafe { dealloc(ptr as *mut u8, layout) }
}

static SERIAL_CONTEXT_ARENA: std::sync::LazyLock<ContextArena<Vec<u8>>> =
    std::sync::LazyLock::new(ContextArena::default);

unsafe extern "C" fn serial_write_callback(ctx: u64, data: *const c_void, len: u64) -> OrcError {
    let incoming: &[u8] = unsafe { slice_from_ptr(data.cast(), len as usize) };
    match SERIAL_CONTEXT_ARENA.visit_mut(ctx, |buf| buf.extend_from_slice(incoming)) {
        Ok(_) => ORC_ERROR_NONE,
        Err(e) => e.into(),
    }
}

unsafe extern "C" fn report_message(
    ctx: u64,
    level: orc_sdk::OrcMessageLevel,
    msg: *const std::ffi::c_char,
) {
    let msg = if msg.is_null() {
        ""
    } else {
        &unsafe { CStr::from_ptr(msg) }.to_string_lossy()
    };
    let level_str = match level {
        orc_sdk::ORC_MSG_LEVEL_DEBUG => "DEBUG",
        orc_sdk::ORC_MSG_LEVEL_INFO => "INFO",
        orc_sdk::ORC_MSG_LEVEL_WARN => "WARN",
        orc_sdk::ORC_MSG_LEVEL_ERROR => "ERROR",
        _ => "FATAL",
    };
    eprintln!("[{level_str}][{ctx}] {msg}");
}

const HOST: OrcHost = OrcHost {
    abi_version: ORC_ABI_VERSION,
    memory_api: OrcHostMemoryAPI {
        alloc: Some(host_alloc),
        dealloc: Some(host_dealloc),
    },
    callbacks: OrcHostCallbackAPI {
        report_progress: None,
        report_message: Some(report_message),
        check_cancellation: None,
        report_intermediate_output: None,
        serial_write: Some(serial_write_callback),
    },
    create_deck_from_proxy: Some(host_create_proxy_deck),
};

// --- Session state ---

struct Session {
    handles: HashMap<u64, OrcHandle>,
    handle_counter: AtomicU64,
    registry: DeckRegistry,
}

impl Session {
    fn new() -> Self {
        Self {
            handles: HashMap::new(),
            handle_counter: AtomicU64::new(1),
            registry: DeckRegistry::new(),
        }
    }

    fn next_id(&self) -> u64 {
        self.handle_counter.fetch_add(1, Ordering::Relaxed)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        for handle in self.handles.values_mut() {
            handle.free();
        }
    }
}

// --- Server ---

pub struct OrcServer {
    thread: Option<JoinHandle<()>>,
}

struct ServerInner {
    plugin_set: PluginSet,
    sessions: Mutex<HashMap<u64, Session>>,
    session_counter: AtomicU64,
}

impl OrcServer {
    pub fn start(plugin_dir: &str, port: u16) -> Result<Self, String> {
        let plugin_set = PluginSet::load_from_dir(std::path::Path::new(plugin_dir), &HOST)
            .map_err(|e| format!("Failed to load plugins: {e}"))?;
        let addr = format!("0.0.0.0:{port}");
        let server = Server::http(&addr).map_err(|e| format!("Failed to bind {addr}: {e}"))?;
        let inner = Arc::new(ServerInner {
            plugin_set,
            sessions: Mutex::new(HashMap::new()),
            session_counter: AtomicU64::new(1),
        });
        let thread = std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let inner = Arc::clone(&inner);
                std::thread::spawn(move || {
                    inner.handle_request(request);
                });
            }
        });
        Ok(OrcServer {
            thread: Some(thread),
        })
    }

    pub fn join(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl ServerInner {
    fn handle_request(&self, mut request: Request) {
        let url = request.url().to_string();
        let method = request.method().clone();
        let result = match (method, url.as_str()) {
            (Method::Post, "/session/start") => self.start_session(),
            (Method::Post, "/session/close") => self.close_session(&mut request),
            (Method::Post, "/constant") => self.create_constant(&mut request),
            (Method::Post, "/call") => self.call_function(&mut request),
            (Method::Get, "/functions") => self.list_functions(),
            (Method::Post, "/download") => self.download_handle(&mut request),
            _ => Err((404, "Not found".to_string())),
        };
        match result {
            Ok(body) => {
                let response = Response::from_string(&body)
                    .with_header(
                        tiny_http::Header::from_bytes(
                            b"Content-Type" as &[u8],
                            b"application/json" as &[u8],
                        )
                        .unwrap(),
                    )
                    .with_status_code(StatusCode(200));
                let _ = request.respond(response);
            }
            Err((code, msg)) => {
                let body = format!(r#"{{"error": "{}"}}"#, msg.replace('"', r#"\""#));
                let response = Response::from_string(&body)
                    .with_header(
                        tiny_http::Header::from_bytes(
                            b"Content-Type" as &[u8],
                            b"application/json" as &[u8],
                        )
                        .unwrap(),
                    )
                    .with_status_code(StatusCode(code as u16));
                let _ = request.respond(response);
            }
        }
    }

    fn read_body(request: &mut Request) -> Result<String, (i32, String)> {
        let mut body = String::new();
        request
            .as_reader()
            .read_to_string(&mut body)
            .map_err(|e| (400, format!("Failed to read body: {e}")))?;
        Ok(body)
    }

    fn parse_json(body: &str) -> Result<JsonValue, (i32, String)> {
        body.parse::<JsonValue>()
            .map_err(|e| (400, format!("Invalid JSON: {e}")))
    }

    fn json_get_u64(obj: &HashMap<String, JsonValue>, key: &str) -> Result<u64, (i32, String)> {
        match obj.get(key) {
            Some(JsonValue::Number(n)) => Ok(*n as u64),
            _ => Err((400, format!("Missing or invalid field: {key}"))),
        }
    }

    fn json_get_str<'a>(
        obj: &'a HashMap<String, JsonValue>,
        key: &str,
    ) -> Result<&'a str, (i32, String)> {
        match obj.get(key) {
            Some(JsonValue::String(s)) => Ok(s.as_str()),
            _ => Err((400, format!("Missing or invalid field: {key}"))),
        }
    }

    // POST /session/start -> {"session_id": <id>}
    fn start_session(&self) -> Result<String, (i32, String)> {
        let id = self.session_counter.fetch_add(1, Ordering::Relaxed);
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(id, Session::new());
        Ok(format!(r#"{{"session_id": {id}}}"#))
    }

    // POST /session/close {"session_id": <id>}
    fn close_session(&self, request: &mut Request) -> Result<String, (i32, String)> {
        let body = Self::read_body(request)?;
        let json = Self::parse_json(&body)?;
        let obj = json_as_object(&json)?;
        let session_id = Self::json_get_u64(obj, "session_id")?;
        let mut sessions = self.sessions.lock().unwrap();
        sessions
            .remove(&session_id)
            .ok_or((404, "Session not found".to_string()))?;
        Ok(r#"{"ok": true}"#.to_string())
    }

    // POST /constant {"session_id": <id>, "type": "f64", "values": [1.0, 2.0, 3.0]}
    // For nested: {"session_id": <id>, "type": "f64", "values": [[1.0, 2.0], [3.0]]}
    fn create_constant(&self, request: &mut Request) -> Result<String, (i32, String)> {
        let body = Self::read_body(request)?;
        let json = Self::parse_json(&body)?;
        let obj = json_as_object(&json)?;
        let session_id = Self::json_get_u64(obj, "session_id")?;
        let type_name = Self::json_get_str(obj, "type")?;
        let values = obj
            .get("values")
            .ok_or((400, "Missing field: values".to_string()))?;
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(&session_id)
            .ok_or((404, "Session not found".to_string()))?;
        let handle_id = session.next_id();
        let mut handle = OrcHandle {
            handle: handle_id,
            ..Default::default()
        };
        match type_name {
            "f64" => create_deck_from_json::<f64>(values, &mut handle, &session.registry)?,
            "f32" => create_deck_from_json::<f32>(values, &mut handle, &session.registry)?,
            "u8" => create_deck_from_json::<u8>(values, &mut handle, &session.registry)?,
            "u16" => create_deck_from_json::<u16>(values, &mut handle, &session.registry)?,
            "u32" => create_deck_from_json::<u32>(values, &mut handle, &session.registry)?,
            "u64" => create_deck_from_json::<u64>(values, &mut handle, &session.registry)?,
            "i8" => create_deck_from_json::<i8>(values, &mut handle, &session.registry)?,
            "i16" => create_deck_from_json::<i16>(values, &mut handle, &session.registry)?,
            "i32" => create_deck_from_json::<i32>(values, &mut handle, &session.registry)?,
            "i64" => create_deck_from_json::<i64>(values, &mut handle, &session.registry)?,
            _ => return Err((400, format!("Unknown type: {type_name}"))),
        }
        session.handles.insert(handle_id, handle);
        Ok(format!(r#"{{"handle_id": {handle_id}}}"#))
    }

    // POST /call {"session_id": <id>, "function": "add", "inputs": [1, 2]}
    fn call_function(&self, request: &mut Request) -> Result<String, (i32, String)> {
        let body = Self::read_body(request)?;
        let json = Self::parse_json(&body)?;
        let obj = json_as_object(&json)?;
        let session_id = Self::json_get_u64(obj, "session_id")?;
        let func_name = Self::json_get_str(obj, "function")?;
        let input_ids = json_as_u64_array(
            obj.get("inputs")
                .ok_or((400, "Missing field: inputs".to_string()))?,
        )?;
        let func_info = self
            .plugin_set
            .get_function(func_name)
            .ok_or((404, format!("Function not found: {func_name}")))?;
        let n_outputs = func_info.n_outputs.unwrap_or(1);
        let func = func_info
            .func
            .ok_or((500, "Invalid function pointer".to_string()))?;
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get_mut(&session_id)
            .ok_or((404, "Session not found".to_string()))?;
        // Gather input handles (copies for the FFI call).
        let inputs: Vec<OrcHandleBorrowed<'_>> = input_ids
            .iter()
            .map(|id| {
                session
                    .handles
                    .get(id)
                    .map(|h| h.borrowed())
                    .ok_or((404, format!("Handle not found: {id}")))
            })
            .collect::<Result<_, _>>()?;
        // Prepare output handles.
        let mut outputs: Vec<OrcHandle> = (0..n_outputs)
            .map(|_| {
                let id = session.next_id();
                OrcHandle {
                    handle: id,
                    ..Default::default()
                }
            })
            .collect();
        let err = unsafe {
            func(
                0,
                inputs.as_ptr().cast(),
                inputs.len() as u64,
                outputs.as_mut_ptr(),
                outputs.len() as u64,
            )
        };
        Error::from_raw(err).map_err(|e| (500, format!("Function call failed: {e}")))?;
        let output_ids: Vec<u64> = outputs.iter().map(|h| h.handle).collect();
        for handle in outputs {
            session.handles.insert(handle.handle, handle);
        }
        let ids_str: Vec<String> = output_ids.iter().map(|id| id.to_string()).collect();
        Ok(format!(r#"{{"output_ids": [{}]}}"#, ids_str.join(", ")))
    }

    // GET /functions -> list of available functions
    fn list_functions(&self) -> Result<String, (i32, String)> {
        let mut entries = Vec::new();
        for plugin in self.plugin_set.plugins() {
            for func in plugin.functions() {
                let name = func.name.replace('"', r#"\""#);
                let desc = func.desc.replace('"', r#"\""#);
                let n_in = match func.n_inputs {
                    Some(n) => n.to_string(),
                    None => "null".to_string(),
                };
                let n_out = match func.n_outputs {
                    Some(n) => n.to_string(),
                    None => "null".to_string(),
                };
                entries.push(format!(
                    r#"{{"name": "{name}", "desc": "{desc}", "n_inputs": {n_in}, "n_outputs": {n_out}}}"#
                ));
            }
        }
        Ok(format!(r#"{{"functions": [{}]}}"#, entries.join(", ")))
    }

    // POST /download {"session_id": <id>, "handle_id": <id>, "type": "f64"}
    fn download_handle(&self, request: &mut Request) -> Result<String, (i32, String)> {
        let body = Self::read_body(request)?;
        let json = Self::parse_json(&body)?;
        let obj = json_as_object(&json)?;
        let session_id = Self::json_get_u64(obj, "session_id")?;
        let handle_id = Self::json_get_u64(obj, "handle_id")?;
        let type_name = Self::json_get_str(obj, "type")?;
        let sessions = self.sessions.lock().unwrap();
        let session = sessions
            .get(&session_id)
            .ok_or((404, "Session not found".to_string()))?;
        let handle = session
            .handles
            .get(&handle_id)
            .ok_or((404, "Handle not found".to_string()))?;
        match type_name {
            "f64" => download_as_json::<f64>(handle),
            "f32" => download_as_json::<f32>(handle),
            "u8" => download_as_json::<u8>(handle),
            "u16" => download_as_json::<u16>(handle),
            "u32" => download_as_json::<u32>(handle),
            "u64" => download_as_json::<u64>(handle),
            "i8" => download_as_json::<i8>(handle),
            "i16" => download_as_json::<i16>(handle),
            "i32" => download_as_json::<i32>(handle),
            "i64" => download_as_json::<i64>(handle),
            _ => Err((400, format!("Unknown type: {type_name}"))),
        }
    }
}

// --- Helpers ---

fn json_as_object(json: &JsonValue) -> Result<&HashMap<String, JsonValue>, (i32, String)> {
    match json {
        JsonValue::Object(obj) => Ok(obj),
        _ => Err((400, "Expected JSON object".to_string())),
    }
}

fn json_as_u64_array(json: &JsonValue) -> Result<Vec<u64>, (i32, String)> {
    match json {
        JsonValue::Array(arr) => arr
            .iter()
            .map(|v| match v {
                JsonValue::Number(n) => Ok(*n as u64),
                _ => Err((400, "Expected array of numbers".to_string())),
            })
            .collect(),
        _ => Err((400, "Expected array".to_string())),
    }
}

trait FromJsonNumber: Sized {
    fn from_f64(v: f64) -> Self;
}

macro_rules! impl_from_json_number {
    ($($t:ty),*) => {
        $(impl FromJsonNumber for $t {
            fn from_f64(v: f64) -> Self { v as $t }
        })*
    }
}

impl_from_json_number!(f64, f32, u8, u16, u32, u64, i8, i16, i32, i64);

fn collect_json_values<T: FromJsonNumber + Default>(
    json: &JsonValue,
    deck: &mut Deck<T>,
    depth: u8,
) -> Result<(), (i32, String)> {
    match json {
        JsonValue::Array(arr) => {
            if arr.is_empty() {
                deck.start_new_arr(depth);
            } else {
                for (i, v) in arr.iter().enumerate() {
                    let child_depth = if i == 0 { depth } else { 0 };
                    collect_json_values(v, deck, child_depth)?;
                }
            }
            Ok(())
        }
        JsonValue::Number(n) => {
            deck.push(T::from_f64(*n), depth);
            Ok(())
        }
        _ => Err((
            400,
            "Values must be numbers or arrays of numbers".to_string(),
        )),
    }
}

fn infer_depth(json: &JsonValue) -> u8 {
    match json {
        JsonValue::Array(arr) => match arr.first() {
            Some(child) => 1 + infer_depth(child),
            None => 1,
        },
        _ => 1,
    }
}

fn create_deck_from_json<T: TOrcData + Any + Send + Sync + Default + FromJsonNumber>(
    values: &JsonValue,
    handle: &mut OrcHandle,
    registry: &DeckRegistry,
) -> Result<(), (i32, String)> {
    let mut deck = Deck::<T>::default();
    let depth = infer_depth(values);
    collect_json_values(values, &mut deck, depth)?;
    registry
        .alloc_with_value(Some(deck), handle)
        .map_err(|e| (500, format!("Failed to allocate deck: {e}")))?;
    Ok(())
}

trait ToJsonNumber {
    fn to_f64(self) -> f64;
}

macro_rules! impl_to_json_number {
    ($($t:ty),*) => {
        $(impl ToJsonNumber for $t {
            fn to_f64(self) -> f64 { self as f64 }
        })*
    }
}

impl_to_json_number!(f64, f32, u8, u16, u32, u64, i8, i16, i32, i64);

fn deck_view_to_json<T: Default + Copy + ToJsonNumber>(view: &DeckView<T>) -> String {
    if view.depth() <= 1 {
        let items: Vec<String> = view
            .as_slice()
            .iter()
            .map(|v| format!("{}", v.to_f64()))
            .collect();
        format!("[{}]", items.join(", "))
    } else {
        let children: Vec<String> = view
            .child()
            .advance_iter()
            .map(|child| deck_view_to_json(&child))
            .collect();
        format!("[{}]", children.join(", "))
    }
}

fn download_as_json<T: TOrcData + Default + Copy + ToJsonNumber>(
    handle: &OrcHandle,
) -> Result<String, (i32, String)> {
    let view = DeckView::<T>::from_handle(handle)
        .map_err(|e| (500, format!("Failed to create deck view: {e}")))?;
    let values = deck_view_to_json(&view);
    Ok(format!(r#"{{"values": {values}}}"#))
}

// --- Proxy deck creation (required by the host) ---

unsafe extern "C" fn host_create_proxy_deck(
    inputs: *const OrcHandle,
    n_inputs: u64,
    proxy_type: OrcProxyType,
    proxy: *const OrcHandle,
    out: *mut OrcHandle,
) -> OrcError {
    if inputs.is_null() || proxy.is_null() || out.is_null() {
        return orc_sdk::ORC_ERROR_INVALID_HANDLE;
    }
    let (inputs, proxy, out) = unsafe {
        (
            slice_from_ptr(inputs, n_inputs as usize),
            &*proxy,
            &mut *out,
        )
    };
    let type_id = match inputs.first() {
        Some(input) => input.type_id,
        None => return ORC_ERROR_INVALID_PROXY,
    };
    if inputs.iter().skip(1).any(|h| h.type_id != type_id) {
        return ORC_ERROR_INVALID_PROXY;
    }
    let proxy_type = match proxy_type {
        ORC_DECK_PROXY_COPY_ALL => ProxyType::CopyAll,
        ORC_DECK_PROXY_COPY_ITEMS => ProxyType::CopyItems,
        ORC_DECK_PROXY_SHUFFLE => ProxyType::Shuffle,
        _ => return ORC_ERROR_INVALID_PROXY,
    };
    // For the server host, we only handle primitive types in the proxy callback.
    // Plugin types are handled by the plugin itself.
    let result = match type_id {
        ORC_TYPE_U8 => {
            orc_sdk::deck_from_proxy::<u8>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_U16 => {
            orc_sdk::deck_from_proxy::<u16>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_U32 => {
            orc_sdk::deck_from_proxy::<u32>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_U64 => {
            orc_sdk::deck_from_proxy::<u64>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_I8 => {
            orc_sdk::deck_from_proxy::<i8>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_I16 => {
            orc_sdk::deck_from_proxy::<i16>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_I32 => {
            orc_sdk::deck_from_proxy::<i32>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_I64 => {
            orc_sdk::deck_from_proxy::<i64>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_F32 => {
            orc_sdk::deck_from_proxy::<f32>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        ORC_TYPE_F64 => {
            orc_sdk::deck_from_proxy::<f64>(inputs, proxy_type, proxy, out, &DECK_REGISTRY)
        }
        _ => return ORC_ERROR_INVALID_PROXY,
    };
    if let Err(e) = result {
        return e.into();
    }
    ORC_ERROR_NONE
}

static DECK_REGISTRY: std::sync::LazyLock<DeckRegistry> =
    std::sync::LazyLock::new(DeckRegistry::new);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orc_deck_free(handle: *mut OrcHandle) -> OrcError {
    if handle.is_null() {
        return ORC_ERROR_NONE;
    }
    let handle = unsafe { &mut *handle };
    match DECK_REGISTRY.free(handle.handle) {
        Ok(()) => {
            reset_handle(handle);
            ORC_ERROR_NONE
        }
        Err(e) => e.into(),
    }
}
