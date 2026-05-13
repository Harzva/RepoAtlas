#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{anyhow, Context, Result};
use include_dir::{include_dir, Dir};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Icon, WindowBuilder},
};
use walkdir::{DirEntry, WalkDir};
use wry::WebViewBuilder;

static PUBLIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/public");
static SEED_INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/seed-inventory.json"
));

const APP_NAME: &str = "RepoAtlas";
const INVENTORY_FILE_NAME: &str = "inventory.json";
const DEFAULT_MAX_DEPTH: usize = 10;

#[derive(Clone)]
struct AppState {
    inventory_path: Arc<PathBuf>,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn main() -> wry::Result<()> {
    let state = AppState {
        inventory_path: Arc::new(inventory_path()),
    };
    if let Err(error) = ensure_inventory(&state.inventory_path) {
        eprintln!("Cannot prepare inventory: {error:#}");
    }

    let port = start_http_server(state).expect("start local HTTP server");
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(APP_NAME)
        .with_inner_size(LogicalSize::new(1440.0, 960.0))
        .with_min_inner_size(LogicalSize::new(1100.0, 720.0))
        .with_window_icon(window_icon())
        .build(&event_loop)
        .expect("create window");

    let url = format!("http://127.0.0.1:{port}/");
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .with_new_window_req_handler(|url, _features| {
            let _ = open_external_url(&url);
            wry::NewWindowResponse::Deny
        })
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}

fn window_icon() -> Option<Icon> {
    Icon::from_rgba(include_bytes!("repoatlas_icon_rgba.bin").to_vec(), 64, 64).ok()
}

fn inventory_path() -> PathBuf {
    if let Some(path) = env::var_os("REPO_ATLAS_DATA") {
        return PathBuf::from(path);
    }
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .unwrap_or_else(env::temp_dir);
    base.join(APP_NAME).join(INVENTORY_FILE_NAME)
}

fn ensure_inventory(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, SEED_INVENTORY)?;
    Ok(())
}

fn read_inventory(path: &Path) -> Result<Value> {
    ensure_inventory(path)?;
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn write_inventory(path: &Path, inventory: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(inventory)?)?;
    Ok(())
}

fn start_http_server(state: AppState) -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = state.clone();
            thread::spawn(move || {
                if let Err(error) = handle_stream(stream, &state) {
                    eprintln!("HTTP request failed: {error:#}");
                }
            });
        }
    });
    Ok(port)
}

fn handle_stream(mut stream: TcpStream, state: &AppState) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let request = read_http_request(&mut stream)?;
    let response = route_request(request, state);
    write_response(&mut stream, response)?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];
    let header_end;
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(anyhow!("empty request"));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_subsequence(&buffer, b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
        if buffer.len() > 1024 * 256 {
            return Err(anyhow!("request headers too large"));
        }
    }

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow!("missing request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("").to_string();
    let raw_path = request_parts.next().unwrap_or("/");
    let path = raw_path.split('?').next().unwrap_or("/").to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);

    while buffer.len() < header_end + content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    let body = buffer[header_end..buffer.len().min(header_end + content_length)].to_vec();
    Ok(HttpRequest { method, path, body })
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

struct HttpResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

fn route_request(request: HttpRequest, state: &AppState) -> HttpResponse {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/inventory") => {
            json_response(200, flatten_inventory_result(&state.inventory_path))
        }
        ("GET", "/api/auth/status") => auth_status_response(&[]),
        ("POST", "/api/auth/status") => auth_status_response(&request.body),
        ("POST", "/api/auth/login") => auth_login_response(&request.body),
        ("POST", "/api/refresh") => refresh_response(&request.body, state),
        ("POST", "/api/open-local") => open_local_response(&request.body, state),
        ("GET", path) if path.starts_with("/reports/") => report_response(path, state),
        ("GET", path) => static_response(path),
        _ => text_response(405, "method not allowed"),
    }
}

fn flatten_inventory_result(path: &Path) -> Value {
    match read_inventory(path).map(|inventory| flatten_inventory(&inventory)) {
        Ok(value) => value,
        Err(error) => {
            json!({ "ok": false, "error": error.to_string(), "summary": {}, "rows": [], "localOnly": [] })
        }
    }
}

fn auth_status_response(body: &[u8]) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    apply_gh_path(&payload);
    json_response(200, auth_status_value())
}

fn auth_login_response(body: &[u8]) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    apply_gh_path(&payload);
    let status = auth_status_value();
    if status
        .get("authenticated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return json_response(
            200,
            json!({ "ok": true, "message": "Already authenticated.", "status": status }),
        );
    }

    if !status
        .get("installed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return json_response(
            500,
            json!({ "ok": false, "error": "GitHub CLI was not found. Install gh or set a custom gh path." }),
        );
    }

    let login = run_with_timeout(
        &gh_command(),
        &[
            "auth",
            "login",
            "--web",
            "--git-protocol",
            "https",
            "--hostname",
            "github.com",
        ],
        None,
        Duration::from_secs(300),
    );
    if !login.status.success() {
        return json_response(
            500,
            json!({
                "ok": false,
                "error": process_message(&login),
                "stdout": String::from_utf8_lossy(&login.stdout).trim().to_string(),
            }),
        );
    }

    let status = auth_status_value();
    json_response(
        200,
        json!({ "ok": true, "message": "GitHub authentication completed.", "status": status }),
    )
}

fn auth_status_value() -> Value {
    let version = run(&gh_command(), &["--version"], None);
    if !version.status.success() {
        return json!({
            "ok": true,
            "installed": false,
            "authenticated": false,
            "ghPath": gh_command(),
            "error": process_message(&version),
        });
    }

    let status = gh_raw("", &["auth", "status", "--hostname", "github.com"]);
    let authenticated = status.status.success();
    let login = if authenticated {
        gh("", &["api", "user", "--jq", ".login"])
            .ok()
            .map(|proc| String::from_utf8_lossy(&proc.stdout).trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
    } else {
        String::new()
    };

    json!({
        "ok": true,
        "installed": true,
        "authenticated": authenticated,
        "login": login,
        "ghPath": gh_command(),
        "message": if authenticated { "GitHub CLI is authenticated." } else { "GitHub CLI is installed but not authenticated." },
        "details": process_message(&status),
    })
}

fn refresh_response(body: &[u8], state: &AppState) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    apply_gh_path(&payload);
    let accounts = request_accounts(&payload);
    let fetch = payload
        .get("fetch")
        .and_then(Value::as_bool)
        .unwrap_or_else(default_fetch);
    let max_depth = payload
        .get("maxDepth")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or_else(default_max_depth);
    let scan_roots = payload
        .get("scanRoots")
        .and_then(Value::as_array)
        .map(|roots| {
            roots
                .iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(default_scan_roots);

    match scan_inventory(&accounts, &scan_roots, max_depth, fetch).and_then(|inventory| {
        write_inventory(&state.inventory_path, &inventory)?;
        Ok(inventory)
    }) {
        Ok(inventory) => {
            let mut value = flatten_inventory(&inventory);
            value
                .as_object_mut()
                .unwrap()
                .insert("ok".into(), Value::Bool(true));
            json_response(200, value)
        }
        Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
    }
}

fn request_accounts(payload: &Value) -> Vec<String> {
    if let Some(accounts) = payload.get("accounts").and_then(Value::as_array) {
        let values = accounts
            .iter()
            .filter_map(Value::as_str)
            .flat_map(split_accounts)
            .collect::<Vec<_>>();
        if !values.is_empty() {
            return unique_account_names(values);
        }
    }

    if let Some(account) = payload.get("account").and_then(Value::as_str) {
        let values = split_accounts(account);
        if !values.is_empty() {
            return unique_account_names(values);
        }
    }

    if let Ok(accounts) = env::var("REPO_ATLAS_ACCOUNTS") {
        let values = split_accounts(&accounts);
        if !values.is_empty() {
            return unique_account_names(values);
        }
    }

    if let Ok(account) = env::var("REPO_ATLAS_ACCOUNT") {
        let values = split_accounts(&account);
        if !values.is_empty() {
            return unique_account_names(values);
        }
    }

    vec![String::new()]
}

fn split_accounts(raw: &str) -> Vec<String> {
    raw.split(|ch: char| matches!(ch, '\n' | '\r' | ';' | ','))
        .filter_map(normalize_account_name)
        .collect()
}

fn normalize_account_name(value: &str) -> Option<String> {
    let clean = value.trim();
    if clean.is_empty() {
        return None;
    }
    match clean.to_ascii_lowercase().as_str() {
        "current" | "current gh" | "current gh login" | "default" | "active gh" => {
            Some(String::new())
        }
        "leave empty for current gh login" => None,
        _ => Some(clean.to_string()),
    }
}

fn unique_account_names(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.to_ascii_lowercase()))
        .collect()
}

fn apply_gh_path(payload: &Value) {
    if let Some(path) = payload.get("ghPath").and_then(Value::as_str).map(str::trim) {
        if path.is_empty() {
            env::remove_var("REPO_ATLAS_GH");
        } else {
            env::set_var("REPO_ATLAS_GH", path);
        }
    }
}

fn gh_command() -> String {
    env::var("REPO_ATLAS_GH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "gh".into())
}

fn open_local_response(body: &[u8], state: &AppState) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    let Some(target) = payload
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
    else {
        return json_response(400, json!({ "ok": false, "error": "Missing path." }));
    };

    let inventory = match read_inventory(&state.inventory_path) {
        Ok(value) => value,
        Err(error) => {
            return json_response(500, json!({ "ok": false, "error": error.to_string() }))
        }
    };
    let allowed = allowed_local_paths(&inventory);
    let target_key = normalize_path_key(&target);
    if !allowed.contains(&target_key) {
        return json_response(
            403,
            json!({ "ok": false, "error": "This path is not part of the scanned inventory." }),
        );
    }
    if !target.exists() {
        return json_response(
            404,
            json!({ "ok": false, "error": "Local path does not exist.", "path": target.to_string_lossy().to_string() }),
        );
    }

    match open_path(&target) {
        Ok(()) => json_response(
            200,
            json!({ "ok": true, "path": target.to_string_lossy().to_string() }),
        ),
        Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
    }
}

fn report_response(path: &str, state: &AppState) -> HttpResponse {
    match path.trim_start_matches("/reports/") {
        "repo-atlas.json" => match read_inventory(&state.inventory_path) {
            Ok(value) => json_response(200, value),
            Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
        },
        "repo-atlas-full.csv" => match read_inventory(&state.inventory_path) {
            Ok(value) => bytes_response(
                200,
                "text/csv; charset=utf-8",
                render_csv(&value).into_bytes(),
            ),
            Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
        },
        "repo-atlas-full.md" => match read_inventory(&state.inventory_path) {
            Ok(value) => bytes_response(
                200,
                "text/markdown; charset=utf-8",
                render_markdown(&value).into_bytes(),
            ),
            Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
        },
        _ => text_response(404, "not found"),
    }
}

fn static_response(path: &str) -> HttpResponse {
    let clean_path = if path == "/" {
        "index.html".to_string()
    } else {
        path.trim_start_matches('/').replace('\\', "/")
    };
    if clean_path.contains("..") {
        return text_response(403, "forbidden");
    }

    match PUBLIC_DIR.get_file(&clean_path) {
        Some(file) => bytes_response(200, mime_type(&clean_path), file.contents().to_vec()),
        None => text_response(404, "not found"),
    }
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> Result<()> {
    let status_text = match response.status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status,
        status_text,
        response.content_type,
        response.body.len()
    )?;
    stream.write_all(&response.body)?;
    Ok(())
}

fn json_response(status: u16, value: Value) -> HttpResponse {
    bytes_response(
        status,
        "application/json; charset=utf-8",
        serde_json::to_vec_pretty(&value).unwrap_or_else(|_| b"{}".to_vec()),
    )
}

fn text_response(status: u16, body: &str) -> HttpResponse {
    bytes_response(
        status,
        "text/plain; charset=utf-8",
        body.as_bytes().to_vec(),
    )
}

fn bytes_response(status: u16, content_type: &'static str, body: Vec<u8>) -> HttpResponse {
    HttpResponse {
        status,
        content_type,
        body,
    }
}

fn mime_type(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn flatten_inventory(inventory: &Value) -> Value {
    let rows_in = inventory
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let local_only_in = inventory
        .get("localOnly")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows = Vec::new();
    let mut status_counts = BTreeMap::<String, u64>::new();
    let mut language_counts = BTreeMap::<String, u64>::new();
    let mut category_counts = BTreeMap::<String, u64>::new();
    let mut public_count = 0_u64;
    let mut private_count = 0_u64;
    let mut fork_count = 0_u64;
    let mut archived_count = 0_u64;
    let mut local_match_count = 0_u64;

    for (index, row) in rows_in.iter().enumerate() {
        let remote = row.get("remote").cloned().unwrap_or_else(|| json!({}));
        let matches = row
            .get("localMatches")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let local_path_text = matches
            .iter()
            .filter_map(|match_item| match_item.get("path").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<Vec<_>>();
        let local_paths = local_path_text
            .iter()
            .cloned()
            .map(Value::from)
            .collect::<Vec<_>>();
        let local_status_list = if matches.is_empty() {
            vec![Value::from("no-local-copy")]
        } else {
            unique_strings(
                matches
                    .iter()
                    .filter_map(|match_item| match_item.get("status").and_then(Value::as_str))
                    .collect(),
            )
            .into_iter()
            .map(Value::from)
            .collect()
        };
        let local_status = normalize_status(&matches);
        *status_counts.entry(local_status.clone()).or_insert(0) += 1;

        let language = remote
            .get("primaryLanguage")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        *language_counts
            .entry(if language.is_empty() {
                "Unknown".into()
            } else {
                language.clone()
            })
            .or_insert(0) += 1;

        let is_private = remote
            .get("isPrivate")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_fork = remote
            .get("isFork")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_archived = remote
            .get("isArchived")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if is_private {
            private_count += 1;
        } else {
            public_count += 1;
        }
        if is_fork {
            fork_count += 1;
        }
        if is_archived {
            archived_count += 1;
        }
        local_match_count += matches.len() as u64;

        let name = remote
            .get("nameWithOwner")
            .and_then(Value::as_str)
            .unwrap_or("");
        let description = remote
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let category = classify_repo(name, description, &local_path_text);
        let category_label = category_label(&category);
        *category_counts.entry(category.clone()).or_insert(0) += 1;
        let repo_key = remote.get("repoKey").and_then(Value::as_str).unwrap_or("");
        let id = if repo_key.is_empty() {
            format!("{name}{index}")
        } else {
            repo_key.to_string()
        }
        .to_lowercase();
        let url = remote.get("url").and_then(Value::as_str).unwrap_or("");

        rows.push(json!({
            "id": id,
            "name": name,
            "owner": remote.get("accountLogin").or_else(|| remote.get("accountAlias")).and_then(Value::as_str).unwrap_or(""),
            "repoKey": repo_key,
            "url": url,
            "visibility": if is_private { "private" } else { "public" },
            "isPrivate": is_private,
            "isFork": is_fork,
            "isArchived": is_archived,
            "language": language,
            "defaultBranch": row.get("defaultBranch").and_then(Value::as_str).unwrap_or(""),
            "pushedAt": remote.get("pushedAt").and_then(Value::as_str).unwrap_or(""),
            "updatedAt": remote.get("updatedAt").and_then(Value::as_str).unwrap_or(""),
            "description": description,
            "category": category,
            "categoryLabel": category_label,
            "localStatus": local_status,
            "localStatusList": local_status_list,
            "localMatchCount": matches.len(),
            "localPaths": local_paths,
            "localMatches": matches,
            "cloneUrl": if url.is_empty() { String::new() } else { format!("{url}.git") },
            "raw": row,
        }));
    }

    let local_only = local_only_in
        .iter()
        .enumerate()
        .map(|(index, local)| {
            let path = local.get("path").and_then(Value::as_str).unwrap_or("");
            let local_context = vec![path.to_string()];
            let remote_text = local
                .get("remotes")
                .and_then(Value::as_array)
                .map(|remotes| {
                    remotes
                        .iter()
                        .filter_map(|remote| {
                            remote
                                .get("repoKey")
                                .or_else(|| remote.get("url"))
                                .and_then(Value::as_str)
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let category = classify_repo(&remote_text, path, &local_context);
            let category_label = category_label(&category);
            json!({
                "id": format!("local-only-{index}"),
                "path": path,
                "branch": local.get("branch").and_then(Value::as_str).unwrap_or(""),
                "status": local.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                "category": category,
                "categoryLabel": category_label,
                "head": local.get("head").and_then(Value::as_str).unwrap_or(""),
                "upstream": local.get("upstream").and_then(Value::as_str).unwrap_or(""),
                "upstreamSha": local.get("upstreamSha").and_then(Value::as_str).unwrap_or(""),
                "ahead": local.get("ahead").cloned().unwrap_or(Value::Null),
                "behind": local.get("behind").cloned().unwrap_or(Value::Null),
                "dirty": local.get("dirty").and_then(Value::as_bool).unwrap_or(false),
                "remotes": local.get("remotes").cloned().unwrap_or_else(|| json!([])),
                "error": local.get("error").and_then(Value::as_str).unwrap_or(""),
                "raw": local,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "summary": {
            "generatedAt": inventory.get("generatedAt").and_then(Value::as_str).unwrap_or(""),
            "remoteCount": inventory.get("remoteCount").and_then(Value::as_u64).unwrap_or(rows.len() as u64),
            "localRepoCount": inventory.get("localRepoCount").and_then(Value::as_u64).unwrap_or(0),
            "matchedRemoteCount": inventory.get("matchedRemoteCount").and_then(Value::as_u64).unwrap_or_else(|| rows.iter().filter(|row| row.get("localMatchCount").and_then(Value::as_u64).unwrap_or(0) > 0).count() as u64),
            "localOnlyCount": local_only.len(),
            "localMatchCount": local_match_count,
            "publicCount": public_count,
            "privateCount": private_count,
            "forkCount": fork_count,
            "archivedCount": archived_count,
            "statusCounts": status_counts,
            "languageCounts": language_counts,
            "categoryCounts": category_counts,
            "scanRoots": inventory.get("scanRoots").cloned().unwrap_or_else(|| json!([])),
            "accounts": inventory.get("accounts").cloned().unwrap_or_else(|| legacy_accounts(inventory)),
            "accountErrors": inventory.get("accountErrors").cloned().unwrap_or_else(|| json!([])),
            "accountAlias": inventory.get("accountAlias").and_then(Value::as_str).unwrap_or(""),
            "accountLogin": inventory.get("accountLogin").and_then(Value::as_str).unwrap_or(""),
            "versionCheckUsedFetch": inventory.get("versionCheckUsedFetch").cloned().unwrap_or(Value::Null),
        },
        "rows": rows,
        "localOnly": local_only,
    })
}

fn normalize_status(matches: &[Value]) -> String {
    if matches.is_empty() {
        return "no-local-copy".into();
    }
    if matches
        .iter()
        .any(|item| item.get("dirty").and_then(Value::as_bool).unwrap_or(false))
    {
        return "dirty".into();
    }
    let statuses = matches
        .iter()
        .filter_map(|item| item.get("status").and_then(Value::as_str))
        .collect::<Vec<_>>();
    for candidate in ["diverged", "behind", "ahead"] {
        if statuses.contains(&candidate) {
            return candidate.into();
        }
    }
    if !statuses.is_empty() && statuses.iter().all(|status| *status == "synced") {
        return "synced".into();
    }
    if statuses.contains(&"no-upstream") {
        return "no-upstream".into();
    }
    statuses.first().copied().unwrap_or("unknown").into()
}

fn unique_strings(values: Vec<&str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert((*value).to_string()))
        .map(str::to_string)
        .collect()
}

fn legacy_accounts(inventory: &Value) -> Value {
    let alias = inventory
        .get("accountAlias")
        .and_then(Value::as_str)
        .unwrap_or("");
    let login = inventory
        .get("accountLogin")
        .and_then(Value::as_str)
        .unwrap_or("");
    if alias.is_empty() && login.is_empty() {
        json!([])
    } else {
        json!([{ "alias": alias, "login": login, "repoCount": inventory.get("remoteCount").and_then(Value::as_u64).unwrap_or(0) }])
    }
}

fn classify_repo(name: &str, description: &str, local_paths: &[String]) -> String {
    let haystack =
        format!("{} {} {}", name, description, local_paths.join(" ")).to_ascii_lowercase();

    if contains_any(
        &haystack,
        &[
            "model-context-protocol",
            "mcp-",
            "-mcp",
            "/mcp",
            " mcp",
            "mcp server",
            "connector",
        ],
    ) {
        return "mcp".into();
    }
    if contains_any(
        &haystack,
        &[
            "skill",
            "skills",
            "codex-skill",
            "agents/skills",
            ".codex/skills",
        ],
    ) {
        return "skills".into();
    }
    if contains_any(
        &haystack,
        &[
            "memory",
            "memories",
            "knowledge",
            "rag",
            "vector",
            "obsidian",
        ],
    ) {
        return "memory".into();
    }
    if contains_any(
        &haystack,
        &[
            "desktop",
            "app",
            "software",
            "cli",
            "tool",
            "extension",
            "market",
            "release",
            "webview",
        ],
    ) {
        return "software".into();
    }
    if contains_any(
        &haystack,
        &[
            "docs",
            "documentation",
            "readme",
            "website",
            "blog",
            "roadmap",
            "course",
        ],
    ) {
        return "docs".into();
    }
    if contains_any(
        &haystack,
        &[
            "action", "workflow", "docker", "deploy", "pipeline", "ci", "router", "config",
        ],
    ) {
        return "infra".into();
    }
    if contains_any(
        &haystack,
        &["dataset", "corpus", "benchmark", "data", "csv", "jsonl"],
    ) {
        return "data".into();
    }
    if contains_any(
        &haystack,
        &[
            "paper",
            "research",
            "model",
            "pytorch",
            "tensorflow",
            "llm",
            "nlp",
            "agent",
        ],
    ) {
        return "research".into();
    }
    if contains_any(&haystack, &["game", "tic-tac-toe", "chess", "puzzle"]) {
        return "games".into();
    }
    "other".into()
}

fn category_label(category: &str) -> &'static str {
    match category {
        "skills" => "Skills",
        "mcp" => "MCP",
        "memory" => "Memory",
        "software" => "Software",
        "docs" => "Docs",
        "infra" => "Infra",
        "data" => "Data",
        "research" => "Research",
        "games" => "Games",
        _ => "Other",
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn allowed_local_paths(inventory: &Value) -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();
    if let Some(rows) = inventory.get("rows").and_then(Value::as_array) {
        for row in rows {
            if let Some(matches) = row.get("localMatches").and_then(Value::as_array) {
                for match_item in matches {
                    if let Some(path) = match_item.get("path").and_then(Value::as_str) {
                        allowed.insert(normalize_path_key(Path::new(path)));
                    }
                }
            }
        }
    }
    if let Some(locals) = inventory.get("localOnly").and_then(Value::as_array) {
        for local in locals {
            if let Some(path) = local.get("path").and_then(Value::as_str) {
                allowed.insert(normalize_path_key(Path::new(path)));
            }
        }
    }
    allowed
}

fn normalize_path_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase()
}

fn default_scan_roots() -> Vec<PathBuf> {
    if let Some(raw) = env::var_os("REPO_ATLAS_SCAN_ROOTS") {
        return env::split_paths(&raw).collect();
    }
    let mut roots = BTreeSet::new();
    roots.insert(env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if let Some(home) = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME")) {
        let home = PathBuf::from(home);
        for candidate in [
            home.join("source").join("repos"),
            home.join("Documents").join("GitHub"),
            home.join("Projects"),
            home.join("repos"),
        ] {
            if candidate.exists() {
                roots.insert(candidate);
            }
        }
    }
    roots.into_iter().collect()
}

fn default_max_depth() -> usize {
    env::var("REPO_ATLAS_MAX_DEPTH")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_DEPTH)
}

fn default_fetch() -> bool {
    let raw = env::var("REPO_ATLAS_NO_FETCH").unwrap_or_default();
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes"
    )
}

fn scan_inventory(
    accounts: &[String],
    scan_roots: &[PathBuf],
    max_depth: usize,
    fetch: bool,
) -> Result<Value> {
    let requested_accounts = if accounts.is_empty() {
        vec![String::new()]
    } else {
        accounts.to_vec()
    };
    let mut remote_repos = Vec::new();
    let mut account_summaries = Vec::new();
    let mut account_errors = Vec::new();
    let mut seen_repo_keys = BTreeSet::new();

    for account in requested_accounts {
        match list_remote_repos(&account) {
            Ok(repos) => {
                let account_login = repos
                    .first()
                    .and_then(|repo| repo.get("accountLogin"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let account_alias = if account.trim().is_empty() {
                    account_login.clone()
                } else {
                    account.trim().to_string()
                };
                let repo_count = repos.len();
                account_summaries.push(json!({
                    "alias": account_alias,
                    "login": account_login,
                    "repoCount": repo_count,
                }));
                for repo in repos {
                    let key = repo
                        .get("repoKey")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    if key.is_empty() || seen_repo_keys.insert(key) {
                        remote_repos.push(repo);
                    }
                }
            }
            Err(error) => {
                account_errors.push(json!({
                    "alias": account.trim(),
                    "error": error.to_string(),
                }));
            }
        }
    }

    if remote_repos.is_empty() && !account_errors.is_empty() {
        return Err(anyhow!(
            "No GitHub repositories could be loaded. {}",
            account_errors
                .iter()
                .filter_map(|item| item.get("error").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    remote_repos.sort_by(|a, b| {
        a.get("nameWithOwner")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("nameWithOwner").and_then(Value::as_str).unwrap_or(""))
    });

    let git_roots = find_git_roots(scan_roots, max_depth);
    let local_repos = git_roots
        .iter()
        .map(|path| inspect_local_repo(path, fetch))
        .collect::<Vec<_>>();
    let mut inventory = merge_inventory(remote_repos, local_repos);
    let account_aliases = account_summaries
        .iter()
        .filter_map(|item| item.get("alias").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let account_logins = account_summaries
        .iter()
        .filter_map(|item| item.get("login").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    inventory
        .as_object_mut()
        .unwrap()
        .insert("accounts".into(), Value::Array(account_summaries));
    inventory
        .as_object_mut()
        .unwrap()
        .insert("accountErrors".into(), Value::Array(account_errors));
    inventory
        .as_object_mut()
        .unwrap()
        .insert("accountAlias".into(), Value::from(account_aliases));
    inventory
        .as_object_mut()
        .unwrap()
        .insert("accountLogin".into(), Value::from(account_logins));
    inventory.as_object_mut().unwrap().insert(
        "scanRoots".into(),
        Value::Array(
            scan_roots
                .iter()
                .map(|root| Value::from(root.to_string_lossy().to_string()))
                .collect(),
        ),
    );
    inventory
        .as_object_mut()
        .unwrap()
        .insert("versionCheckUsedFetch".into(), Value::Bool(fetch));
    Ok(inventory)
}

fn list_remote_repos(account: &str) -> Result<Vec<Value>> {
    let account = account.trim();
    let routed = router_path().exists() && !account.is_empty();
    let login_text = if account.is_empty() || routed {
        let user = gh(account, &["api", "user", "--jq", ".login"])?;
        String::from_utf8_lossy(&user.stdout).trim().to_string()
    } else {
        account.to_string()
    };
    let login = login_text.as_str();
    let fields = "nameWithOwner,url,description,isPrivate,isArchived,isFork,primaryLanguage,pushedAt,updatedAt,defaultBranchRef";
    let repo_proc = gh_raw(
        if routed { account } else { "" },
        &["repo", "list", login, "--limit", "1000", "--json", fields],
    );
    let repo_items = if repo_proc.status.success() {
        serde_json::from_slice::<Vec<Value>>(&repo_proc.stdout)?
    } else {
        list_remote_repos_rest(account, login, routed)?
    };

    let mut seen = BTreeSet::new();
    let mut remotes = Vec::new();
    for mut repo in repo_items {
        let Some(key) = repo
            .get("nameWithOwner")
            .and_then(Value::as_str)
            .and_then(normalize_repo_key)
        else {
            continue;
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        let object = repo
            .as_object_mut()
            .ok_or_else(|| anyhow!("invalid repo item"))?;
        object.insert(
            "accountAlias".into(),
            Value::from(if account.trim().is_empty() {
                login
            } else {
                account
            }),
        );
        object.insert("accountLogin".into(), Value::from(login));
        object.insert("repoKey".into(), Value::from(key));
        remotes.push(repo);
    }
    remotes.sort_by(|a, b| {
        a.get("nameWithOwner")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("nameWithOwner").and_then(Value::as_str).unwrap_or(""))
    });
    Ok(remotes)
}

fn list_remote_repos_rest(account: &str, login: &str, routed: bool) -> Result<Vec<Value>> {
    let jq = ".[] | {nameWithOwner:.full_name,url:.html_url,description:.description,isPrivate:.private,isArchived:.archived,isFork:.fork,primaryLanguage:(if .language == null then null else {name:.language} end),pushedAt:.pushed_at,updatedAt:.updated_at,defaultBranchRef:{name:.default_branch}}";
    let route_account = if routed || account.trim().is_empty() {
        account
    } else {
        ""
    };
    let endpoint = if routed || account.trim().is_empty() {
        "/user/repos?affiliation=owner&per_page=100".to_string()
    } else {
        format!("/users/{login}/repos?per_page=100")
    };
    let proc = gh(
        route_account,
        &["api", "--paginate", endpoint.as_str(), "--jq", jq],
    )?;
    let mut repos = Vec::new();
    for line in String::from_utf8_lossy(&proc.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        repos.push(serde_json::from_str(line)?);
    }
    Ok(repos)
}

struct ProcOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn gh(account: &str, args: &[&str]) -> Result<ProcOutput> {
    let output = gh_raw(account, args);
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr)
                .trim()
                .to_string()
                .if_empty_then(String::from_utf8_lossy(&output.stdout).trim().to_string())
        ));
    }
    Ok(output)
}

fn gh_raw(account: &str, args: &[&str]) -> ProcOutput {
    let router = router_path();
    let output = if router.exists() && !account.trim().is_empty() {
        let mut command = Command::new("python");
        command.arg(router).arg("--account").arg(account).arg("--");
        for arg in args {
            command.arg(arg);
        }
        configure_command(&mut command);
        command.output()
    } else {
        let mut command = Command::new(gh_command());
        command.args(args);
        configure_command(&mut command);
        command.output()
    };
    match output {
        Ok(output) => ProcOutput {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        },
        Err(error) => ProcOutput {
            status: failed_status(),
            stdout: vec![],
            stderr: error.to_string().into_bytes(),
        },
    }
}

fn process_message(output: &ProcOutput) -> String {
    String::from_utf8_lossy(&output.stderr)
        .trim()
        .to_string()
        .if_empty_then(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

trait EmptyFallback {
    fn if_empty_then(self, fallback: String) -> String;
}

impl EmptyFallback for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

#[cfg(windows)]
fn failed_status() -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1)
}

#[cfg(unix)]
fn failed_status() -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(1)
}

fn router_path() -> PathBuf {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("skills")
        .join("gh-account-router")
        .join("scripts")
        .join("gh_account_router.py")
}

fn normalize_repo_key(value: &str) -> Option<String> {
    let mut raw = value.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    if let Some(rest) = raw.strip_prefix("git@github.com:") {
        raw = rest.to_string();
    } else if let Some(rest) = raw.strip_prefix("ssh://git@github.com/") {
        raw = rest.to_string();
    } else if raw.starts_with("https://") || raw.starts_with("http://") {
        let without_scheme = raw.split_once("://")?.1;
        let mut pieces = without_scheme.splitn(2, '/');
        let host = pieces.next()?.to_lowercase();
        if host != "github.com" {
            return None;
        }
        raw = pieces.next().unwrap_or("").to_string();
    }
    if let Some(rest) = raw.strip_suffix(".git") {
        raw = rest.to_string();
    }
    let parts = raw.trim_matches('/').split('/').collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    Some(format!(
        "{}/{}",
        parts[0].to_lowercase(),
        parts[1].to_lowercase()
    ))
}

fn find_git_roots(scan_roots: &[PathBuf], max_depth: usize) -> Vec<PathBuf> {
    let mut repos = BTreeSet::new();
    for root in scan_roots.iter().filter(|root| root.exists()) {
        let walker = WalkDir::new(root)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|entry| !should_skip_entry(entry));
        for entry in walker.flatten().filter(|entry| entry.file_type().is_dir()) {
            if entry.path().join(".git").exists() {
                if let Some(top) = git_output(entry.path(), &["rev-parse", "--show-toplevel"]) {
                    repos.insert(PathBuf::from(top));
                }
            }
        }
    }
    repos.into_iter().collect()
}

fn should_skip_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | ".next"
            | ".nuxt"
            | ".venv"
            | "venv"
            | "__pycache__"
            | "dist"
            | "build"
            | "target"
            | ".cache"
    ) && !name.starts_with(".cache")
}

fn inspect_local_repo(repo_path: &Path, fetch: bool) -> Value {
    let mut status = "unknown".to_string();
    let mut error = Value::Null;

    if fetch {
        let _ = run(
            "git",
            &["fetch", "--all", "--prune", "--quiet"],
            Some(repo_path),
        );
    }

    let branch = git_output(repo_path, &["branch", "--show-current"]);
    let head = git_output(repo_path, &["rev-parse", "HEAD"]);
    let upstream = git_output(
        repo_path,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    );
    let upstream_sha = upstream
        .as_ref()
        .and_then(|_| git_output(repo_path, &["rev-parse", "@{u}"]));
    let dirty = git_output(repo_path, &["status", "--porcelain"])
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let remotes = parse_remotes(repo_path);
    let mut ahead = Value::Null;
    let mut behind = Value::Null;

    if upstream.is_none() {
        status = "no-upstream".into();
    } else if let Some(counts) = git_output(
        repo_path,
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
    ) {
        let numbers = counts
            .split_whitespace()
            .filter_map(|value| value.parse::<u64>().ok())
            .collect::<Vec<_>>();
        if numbers.len() >= 2 {
            ahead = Value::from(numbers[0]);
            behind = Value::from(numbers[1]);
            status = match (numbers[0], numbers[1]) {
                (0, 0) => "synced",
                (a, b) if a > 0 && b > 0 => "diverged",
                (a, _) if a > 0 => "ahead",
                (_, b) if b > 0 => "behind",
                _ => "unknown",
            }
            .into();
        }
    } else {
        status = "error".into();
        error = Value::from("Cannot compare HEAD with upstream");
    }

    json!({
        "path": repo_path.to_string_lossy(),
        "branch": branch,
        "head": head,
        "remotes": remotes,
        "upstream": upstream,
        "upstreamSha": upstream_sha,
        "ahead": ahead,
        "behind": behind,
        "dirty": dirty,
        "status": status,
        "error": error,
    })
}

fn parse_remotes(repo_path: &Path) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut remotes = Vec::new();
    let Some(text) = git_output(repo_path, &["remote", "-v"]) else {
        return remotes;
    };
    for line in text.lines().filter(|line| line.contains("(fetch)")) {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        let key = format!("{} {}", parts[0], parts[1]);
        if !seen.insert(key) {
            continue;
        }
        remotes.push(json!({
            "name": parts[0],
            "url": parts[1],
            "repoKey": normalize_repo_key(parts[1]),
        }));
    }
    remotes
}

fn git_output(repo_path: &Path, args: &[&str]) -> Option<String> {
    let output = run("git", args, Some(repo_path));
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn run(command: &str, args: &[&str], cwd: Option<&Path>) -> ProcOutput {
    let mut cmd = Command::new(command);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    configure_command(&mut cmd);
    match cmd.output() {
        Ok(output) => ProcOutput {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        },
        Err(error) => ProcOutput {
            status: failed_status(),
            stdout: vec![],
            stderr: error.to_string().into_bytes(),
        },
    }
}

fn run_with_timeout(
    command: &str,
    args: &[&str],
    cwd: Option<&Path>,
    timeout: Duration,
) -> ProcOutput {
    let mut cmd = Command::new(command);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    configure_command(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            return ProcOutput {
                status: failed_status(),
                stdout: vec![],
                stderr: error.to_string().into_bytes(),
            }
        }
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return match child.wait_with_output() {
                    Ok(output) => ProcOutput {
                        status: output.status,
                        stdout: output.stdout,
                        stderr: output.stderr,
                    },
                    Err(error) => ProcOutput {
                        status: failed_status(),
                        stdout: vec![],
                        stderr: error.to_string().into_bytes(),
                    },
                };
            }
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return ProcOutput {
                    status: failed_status(),
                    stdout: vec![],
                    stderr: b"GitHub login timed out before authentication completed.".to_vec(),
                };
            }
            Ok(None) => thread::sleep(Duration::from_millis(250)),
            Err(error) => {
                return ProcOutput {
                    status: failed_status(),
                    stdout: vec![],
                    stderr: error.to_string().into_bytes(),
                }
            }
        }
    }
}

fn configure_command(command: &mut Command) {
    configure_no_window(command);
}

#[cfg(windows)]
fn configure_no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_no_window(_command: &mut Command) {}

fn merge_inventory(remote_repos: Vec<Value>, local_repos: Vec<Value>) -> Value {
    let mut locals_by_key: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for local in &local_repos {
        if let Some(remotes) = local.get("remotes").and_then(Value::as_array) {
            for remote in remotes {
                if let Some(key) = remote.get("repoKey").and_then(Value::as_str) {
                    locals_by_key
                        .entry(key.to_string())
                        .or_default()
                        .push(local.clone());
                }
            }
        }
    }

    let mut rows = Vec::new();
    let mut matched = 0_u64;
    for repo in &remote_repos {
        let key = repo.get("repoKey").and_then(Value::as_str).unwrap_or("");
        let matches = locals_by_key.get(key).cloned().unwrap_or_default();
        if !matches.is_empty() {
            matched += 1;
        }
        let default_branch = repo
            .get("defaultBranchRef")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("");
        rows.push(json!({
            "remote": repo,
            "localMatches": matches,
            "localStatus": rows_local_status(&locals_by_key, key),
            "defaultBranch": default_branch,
        }));
    }

    let remote_keys = remote_repos
        .iter()
        .filter_map(|repo| repo.get("repoKey").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let local_only = local_repos
        .into_iter()
        .filter(|local| {
            local
                .get("remotes")
                .and_then(Value::as_array)
                .map(|remotes| {
                    !remotes.iter().any(|remote| {
                        remote
                            .get("repoKey")
                            .and_then(Value::as_str)
                            .map(|key| remote_keys.contains(key))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    json!({
        "generatedAt": chrono::Utc::now().to_rfc3339(),
        "remoteCount": remote_repos.len(),
        "localRepoCount": rows_local_count(&rows, &local_only),
        "matchedRemoteCount": matched,
        "rows": rows,
        "localOnly": local_only,
    })
}

fn rows_local_status(locals_by_key: &BTreeMap<String, Vec<Value>>, key: &str) -> String {
    locals_by_key
        .get(key)
        .and_then(|matches| matches.first())
        .and_then(|local| local.get("status").and_then(Value::as_str))
        .unwrap_or("no-local-copy")
        .to_string()
}

fn rows_local_count(rows: &[Value], local_only: &[Value]) -> usize {
    let matched = rows
        .iter()
        .filter_map(|row| row.get("localMatches").and_then(Value::as_array))
        .map(Vec::len)
        .sum::<usize>();
    matched + local_only.len()
}

fn render_csv(inventory: &Value) -> String {
    let flattened = flatten_inventory(inventory);
    let rows = flattened
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let headers = [
        "Repository",
        "Account",
        "Category",
        "URL",
        "Visibility",
        "Fork",
        "Archived",
        "Language",
        "Default branch",
        "Local status",
        "Local paths",
        "Last pushed",
        "Updated",
        "Description",
    ];
    let mut lines = vec![headers.join(",")];
    for repo in rows {
        let local_status = repo
            .get("localStatusList")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        let local_paths = repo
            .get("localPaths")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        let record = [
            repo.get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("owner")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("categoryLabel")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("visibility")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            yes_no(repo.get("isFork").and_then(Value::as_bool).unwrap_or(false)).into(),
            yes_no(
                repo.get("isArchived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
            .into(),
            repo.get("language")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("defaultBranch")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            local_status,
            local_paths,
            repo.get("pushedAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("updatedAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            repo.get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ];
        lines.push(
            record
                .iter()
                .map(|value| csv_cell(value))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    lines.join("\n")
}

fn render_markdown(inventory: &Value) -> String {
    let flattened = flatten_inventory(inventory);
    let summary = flattened
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let rows = flattened
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut lines = vec![
        "# GitHub Repository Inventory".into(),
        "".into(),
        format!("Generated: {}", summary.get("generatedAt").and_then(Value::as_str).unwrap_or("")),
        "".into(),
        format!(
            "Total remote repositories: {}",
            summary.get("remoteCount").and_then(Value::as_u64).unwrap_or(0)
        ),
        "".into(),
        "| Repository | Account | Category | Visibility | Fork | Language | Default branch | Local status | Last pushed | Description |".into(),
        "|---|---|---|---|---:|---|---|---|---|---|".into(),
    ];
    for repo in rows {
        let name = repo.get("name").and_then(Value::as_str).unwrap_or("");
        let url = repo.get("url").and_then(Value::as_str).unwrap_or("");
        let link = if url.is_empty() {
            md_cell(name)
        } else {
            format!("[{}]({})", md_cell(name), url)
        };
        let status = repo
            .get("localStatusList")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            link,
            md_cell(repo.get("owner").and_then(Value::as_str).unwrap_or("")),
            md_cell(
                repo.get("categoryLabel")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
            md_cell(repo.get("visibility").and_then(Value::as_str).unwrap_or("")),
            yes_no(repo.get("isFork").and_then(Value::as_bool).unwrap_or(false)),
            md_cell(
                repo.get("language")
                    .and_then(Value::as_str)
                    .unwrap_or("none")
            ),
            md_cell(
                repo.get("defaultBranch")
                    .and_then(Value::as_str)
                    .unwrap_or("none")
            ),
            md_cell(&status),
            md_cell(repo.get("pushedAt").and_then(Value::as_str).unwrap_or("")),
            md_cell(
                repo.get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
        ));
    }
    format!("{}\n", lines.join("\n"))
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn csv_cell(value: &str) -> String {
    let clean = value.replace(['\r', '\n'], " ");
    if clean.contains(',') || clean.contains('"') {
        format!("\"{}\"", clean.replace('"', "\"\""))
    } else {
        clean
    }
}

fn md_cell(value: &str) -> String {
    value.replace(['\r', '\n'], " ").replace('|', "\\|")
}

fn open_path(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .context("open path")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .context("open path")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .context("open path")?;
    }
    Ok(())
}

fn open_external_url(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(url)
            .spawn()
            .context("open url")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().context("open url")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("open url")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_accounts_keeps_current_and_multiple_aliases() {
        let payload = json!({
            "accounts": ["current gh login", "Harzva\nsaihao", "harzva"]
        });
        assert_eq!(
            request_accounts(&payload),
            vec!["".to_string(), "Harzva".to_string(), "saihao".to_string()]
        );
    }

    #[test]
    fn classify_context_categories() {
        assert_eq!(
            classify_repo("owner/agent-skills", "Codex skill pack", &[]),
            "skills"
        );
        assert_eq!(
            classify_repo("owner/local-mcp-server", "Model context protocol", &[]),
            "mcp"
        );
        assert_eq!(
            classify_repo("owner/memory-bank", "RAG knowledge store", &[]),
            "memory"
        );
    }

    #[test]
    fn flatten_inventory_keeps_remote_rows_visible() {
        let inventory = json!({
            "generatedAt": "2026-05-13T00:00:00Z",
            "remoteCount": 2,
            "localRepoCount": 1,
            "matchedRemoteCount": 1,
            "rows": [
                {
                    "remote": {
                        "nameWithOwner": "RepoAtlas/example-skill",
                        "repoKey": "repoatlas/example-skill",
                        "url": "https://github.com/RepoAtlas/example-skill",
                        "description": "Codex skill examples",
                        "isPrivate": false,
                        "isFork": false,
                        "isArchived": false,
                        "primaryLanguage": { "name": "Rust" },
                        "defaultBranchRef": { "name": "main" },
                        "accountLogin": "RepoAtlas"
                    },
                    "defaultBranch": "main",
                    "localMatches": [
                        {
                            "path": "D:\\code\\example-skill",
                            "status": "synced",
                            "dirty": false
                        }
                    ]
                },
                {
                    "remote": {
                        "nameWithOwner": "RepoAtlas/context-mcp",
                        "repoKey": "repoatlas/context-mcp",
                        "url": "https://github.com/RepoAtlas/context-mcp",
                        "description": "MCP server",
                        "isPrivate": false,
                        "isFork": false,
                        "isArchived": false,
                        "primaryLanguage": { "name": "TypeScript" },
                        "accountLogin": "RepoAtlas"
                    },
                    "localMatches": []
                }
            ],
            "localOnly": []
        });

        let flattened = flatten_inventory(&inventory);
        let rows = flattened
            .get("rows")
            .and_then(Value::as_array)
            .expect("flattened rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            flattened
                .get("summary")
                .and_then(|summary| summary.get("remoteCount"))
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            rows[0].get("category").and_then(Value::as_str),
            Some("skills")
        );
        assert_eq!(rows[1].get("category").and_then(Value::as_str), Some("mcp"));
    }
}
