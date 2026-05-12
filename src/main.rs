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
    time::Duration,
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use walkdir::{DirEntry, WalkDir};
use wry::WebViewBuilder;

static PUBLIC_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/public");
static SEED_INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/harzva-github-repos.json"
));

const DEFAULT_ACCOUNT: &str = "Harzva";
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
        .with_title("Harzva Repo Atlas")
        .with_inner_size(LogicalSize::new(1440.0, 960.0))
        .with_min_inner_size(LogicalSize::new(1100.0, 720.0))
        .build(&event_loop)
        .expect("create window");

    let url = format!("http://127.0.0.1:{port}/");
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .with_new_window_req_handler(|url, _features| {
            let _ = open_external_url(url);
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

fn inventory_path() -> PathBuf {
    if let Some(path) = env::var_os("HARZVA_REPO_ATLAS_DATA") {
        return PathBuf::from(path);
    }
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .unwrap_or_else(env::temp_dir);
    base.join("Harzva Repo Atlas")
        .join("harzva-github-repos.json")
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

fn refresh_response(body: &[u8], state: &AppState) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    let account = payload
        .get("account")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_ACCOUNT);
    let fetch = payload
        .get("fetch")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let max_depth = payload
        .get("maxDepth")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_MAX_DEPTH);
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

    match scan_inventory(account, &scan_roots, max_depth, fetch).and_then(|inventory| {
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
        "harzva-github-repos.json" => match read_inventory(&state.inventory_path) {
            Ok(value) => json_response(200, value),
            Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
        },
        "harzva-github-repos-full.csv" => match read_inventory(&state.inventory_path) {
            Ok(value) => bytes_response(
                200,
                "text/csv; charset=utf-8",
                render_csv(&value).into_bytes(),
            ),
            Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
        },
        "harzva-github-repos-full.md" => match read_inventory(&state.inventory_path) {
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
        let local_paths = matches
            .iter()
            .filter_map(|match_item| match_item.get("path").and_then(Value::as_str))
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
            "description": remote.get("description").and_then(Value::as_str).unwrap_or(""),
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
            json!({
                "id": format!("local-only-{index}"),
                "path": local.get("path").and_then(Value::as_str).unwrap_or(""),
                "branch": local.get("branch").and_then(Value::as_str).unwrap_or(""),
                "status": local.get("status").and_then(Value::as_str).unwrap_or("unknown"),
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
            "scanRoots": inventory.get("scanRoots").cloned().unwrap_or_else(|| json!([])),
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
    if let Some(raw) = env::var_os("HARZVA_REPO_SCAN_ROOTS") {
        return env::split_paths(&raw).collect();
    }
    let mut roots = vec![env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
    let study_code = PathBuf::from(r"D:\study\code");
    if study_code.exists() {
        roots.push(study_code);
    }
    roots
}

fn scan_inventory(
    account: &str,
    scan_roots: &[PathBuf],
    max_depth: usize,
    fetch: bool,
) -> Result<Value> {
    let remote_repos = list_remote_repos(account)?;
    let git_roots = find_git_roots(scan_roots, max_depth);
    let local_repos = git_roots
        .iter()
        .map(|path| inspect_local_repo(path, fetch))
        .collect::<Vec<_>>();
    let mut inventory = merge_inventory(remote_repos, local_repos);
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
    let user = gh(account, &["api", "user", "--jq", ".login"])?;
    let login = user.stdout.trim();
    let fields = "nameWithOwner,url,description,isPrivate,isArchived,isFork,primaryLanguage,pushedAt,updatedAt,defaultBranchRef";
    let repo_proc = gh_raw(
        account,
        &["repo", "list", login, "--limit", "1000", "--json", fields],
    );
    let repo_items = if repo_proc.status.success() {
        serde_json::from_slice::<Vec<Value>>(&repo_proc.stdout)?
    } else {
        list_remote_repos_rest(account)?
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
        object.insert("accountAlias".into(), Value::from(account));
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

fn list_remote_repos_rest(account: &str) -> Result<Vec<Value>> {
    let jq = ".[] | {nameWithOwner:.full_name,url:.html_url,description:.description,isPrivate:.private,isArchived:.archived,isFork:.fork,primaryLanguage:(if .language == null then null else {name:.language} end),pushedAt:.pushed_at,updatedAt:.updated_at,defaultBranchRef:{name:.default_branch}}";
    let proc = gh(
        account,
        &[
            "api",
            "--paginate",
            "/user/repos?affiliation=owner&per_page=100",
            "--jq",
            jq,
        ],
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
    let output = if router.exists() {
        let mut command = Command::new("python");
        command.arg(router).arg("--account").arg(account).arg("--");
        for arg in args {
            command.arg(arg);
        }
        command.output()
    } else {
        Command::new("gh").args(args).output()
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
        "# Harzva GitHub Repositories".into(),
        "".into(),
        format!("Generated: {}", summary.get("generatedAt").and_then(Value::as_str).unwrap_or("")),
        "".into(),
        format!(
            "Total remote repositories: {}",
            summary.get("remoteCount").and_then(Value::as_u64).unwrap_or(0)
        ),
        "".into(),
        "| Repository | Visibility | Fork | Language | Default branch | Local status | Last pushed | Description |".into(),
        "|---|---|---:|---|---|---|---|---|".into(),
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
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            link,
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
