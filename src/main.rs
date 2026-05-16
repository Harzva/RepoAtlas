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
    sync::{Arc, OnceLock},
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
static GIT_COMMAND: OnceLock<PathBuf> = OnceLock::new();

const APP_NAME: &str = "RepoAtlas";
const INVENTORY_FILE_NAME: &str = "inventory.json";
const DEFAULT_MAX_DEPTH: usize = 10;
const DEFAULT_REMOTE_WORKERS: usize = 4;
const DEFAULT_LOCAL_SCAN_WORKERS: usize = 8;
const DEFAULT_LOCAL_GIT_WORKERS: usize = 6;

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
        ("POST", "/api/auth/token-login") => auth_token_login_response(&request.body),
        ("POST", "/api/repo-details") => repo_details_response(&request.body),
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
            json!({ "ok": false, "error": error.to_string(), "summary": {}, "rows": [], "localOnly": [], "localProjects": [] })
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
    let force = payload
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = auth_status_value();
    if !force
        && status
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
            "--scopes",
            "read:packages",
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

fn auth_token_login_response(body: &[u8]) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    apply_gh_path(&payload);
    let token = payload
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if token.is_empty() {
        return json_response(
            400,
            json!({ "ok": false, "error": "Token is required for access login." }),
        );
    }

    let version = run(&gh_command(), &["--version"], None);
    if !version.status.success() {
        return json_response(
            500,
            json!({ "ok": false, "error": "GitHub CLI was not found. Install gh or set a custom gh path." }),
        );
    }

    let login = run_with_input(
        &gh_command(),
        &[
            "auth",
            "login",
            "--with-token",
            "--git-protocol",
            "https",
            "--hostname",
            "github.com",
        ],
        None,
        format!("{token}\n").as_bytes(),
    );
    if !login.status.success() {
        return json_response(
            500,
            json!({ "ok": false, "error": process_message(&login) }),
        );
    }

    let status = auth_status_value();
    json_response(
        200,
        json!({ "ok": true, "message": "GitHub token access saved by GitHub CLI.", "status": status }),
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
    let accounts = auth_accounts_value();
    let login = if authenticated {
        gh("", &["api", "user", "--jq", ".login"])
            .ok()
            .map(|proc| String::from_utf8_lossy(&proc.stdout).trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
    } else {
        accounts
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("active").and_then(Value::as_bool).unwrap_or(false))
            })
            .and_then(|item| item.get("login").and_then(Value::as_str))
            .unwrap_or("")
            .to_string()
    };

    json!({
        "ok": true,
        "installed": true,
        "authenticated": authenticated,
        "login": login,
        "accounts": accounts,
        "ghPath": gh_command(),
        "message": if authenticated { "GitHub CLI is authenticated." } else { "GitHub CLI is installed but not authenticated." },
        "details": process_message(&status),
    })
}

fn auth_accounts_value() -> Value {
    let status = gh_raw(
        "",
        &[
            "auth",
            "status",
            "--hostname",
            "github.com",
            "--json",
            "hosts",
        ],
    );
    if !status.status.success() && status.stdout.is_empty() {
        return json!([]);
    }
    let Ok(value) = serde_json::from_slice::<Value>(&status.stdout) else {
        return json!([]);
    };
    value
        .get("hosts")
        .and_then(|hosts| hosts.get("github.com"))
        .and_then(Value::as_array)
        .map(|accounts| {
            Value::Array(
                accounts
                    .iter()
                    .map(|account| {
                        json!({
                            "login": account.get("login").and_then(Value::as_str).unwrap_or(""),
                            "active": account.get("active").and_then(Value::as_bool).unwrap_or(false),
                            "state": account.get("state").and_then(Value::as_str).unwrap_or("unknown"),
                            "gitProtocol": account.get("gitProtocol").and_then(Value::as_str).unwrap_or(""),
                            "tokenSource": account.get("tokenSource").and_then(Value::as_str).unwrap_or(""),
                            "scopes": account.get("scopes").and_then(Value::as_str).unwrap_or(""),
                        })
                    })
                    .collect(),
            )
        })
        .unwrap_or_else(|| json!([]))
}

fn repo_details_response(body: &[u8]) -> HttpResponse {
    let payload: Value = serde_json::from_slice(body).unwrap_or_else(|_| json!({}));
    apply_gh_path(&payload);
    let full_name = payload
        .get("fullName")
        .or_else(|| payload.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let repo_key = payload
        .get("repoKey")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let target = if !full_name.is_empty() && full_name.contains('/') {
        full_name.to_string()
    } else {
        repo_key.to_string()
    };
    if target.split('/').count() < 2 {
        return json_response(
            400,
            json!({ "ok": false, "error": "A GitHub owner/repo name is required." }),
        );
    }
    let account = payload
        .get("account")
        .or_else(|| payload.get("owner"))
        .and_then(Value::as_str)
        .unwrap_or("");

    match load_repo_details(account, &target) {
        Ok(value) => json_response(200, value),
        Err(error) => json_response(500, json!({ "ok": false, "error": error.to_string() })),
    }
}

fn load_repo_details(account: &str, full_name: &str) -> Result<Value> {
    let parts = full_name.split('/').collect::<Vec<_>>();
    if parts.len() < 2 {
        return Err(anyhow!("invalid repository name"));
    }
    let owner = parts[0];
    let name = parts[1];
    let repo_path = format!("repos/{owner}/{name}");
    let (repo, issues, pulls, releases, deployments, pages, packages) = thread::scope(|scope| {
        let repo_handle = scope.spawn(|| {
            gh_api_json(account, vec![repo_path.clone()]).unwrap_or_else(|error| {
                json!({
                    "full_name": full_name,
                    "html_url": format!("https://github.com/{full_name}"),
                    "error": error.to_string(),
                })
            })
        });
        let issues_handle = scope.spawn(|| search_repo_items(account, full_name, "issue"));
        let pulls_handle = scope.spawn(|| search_repo_items(account, full_name, "pr"));
        let releases_handle = scope.spawn(|| {
            gh_api_json(
                account,
                vec![
                    "--method".into(),
                    "GET".into(),
                    format!("{repo_path}/releases"),
                    "-F".into(),
                    "per_page=5".into(),
                ],
            )
            .unwrap_or_else(|error| json!({ "error": error.to_string(), "items": [] }))
        });
        let deployments_handle = scope.spawn(|| {
            gh_api_json(
                account,
                vec![
                    "--method".into(),
                    "GET".into(),
                    format!("{repo_path}/deployments"),
                    "-F".into(),
                    "per_page=5".into(),
                ],
            )
            .unwrap_or_else(|error| json!({ "error": error.to_string(), "items": [] }))
        });
        let pages_handle = scope.spawn(|| {
            gh_api_json(account, vec![format!("{repo_path}/pages")])
                .map(|value| json!({ "enabled": true, "data": value }))
                .unwrap_or_else(|error| json!({ "enabled": false, "error": error.to_string() }))
        });
        let packages_handle = scope.spawn(|| {
            repo_packages_graphql(account, owner, name).unwrap_or_else(
                |error| json!({ "totalCount": 0, "items": [], "error": error.to_string() }),
            )
        });

        (
            repo_handle.join().unwrap_or_else(|_| {
                json!({
                    "full_name": full_name,
                    "html_url": format!("https://github.com/{full_name}"),
                    "error": "repository request panicked",
                })
            }),
            issues_handle.join().unwrap_or_else(
                |_| json!({ "count": 0, "items": [], "error": "issues request panicked" }),
            ),
            pulls_handle.join().unwrap_or_else(
                |_| json!({ "count": 0, "items": [], "error": "pull requests request panicked" }),
            ),
            releases_handle
                .join()
                .unwrap_or_else(|_| json!({ "error": "releases request panicked", "items": [] })),
            deployments_handle.join().unwrap_or_else(
                |_| json!({ "error": "deployments request panicked", "items": [] }),
            ),
            pages_handle
                .join()
                .unwrap_or_else(|_| json!({ "enabled": false, "error": "pages request panicked" })),
            packages_handle.join().unwrap_or_else(
                |_| json!({ "totalCount": 0, "items": [], "error": "packages request panicked" }),
            ),
        )
    });

    let repo_url = repo
        .get("html_url")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{full_name}"));

    Ok(json!({
        "ok": true,
        "repo": {
            "fullName": repo.get("full_name").and_then(Value::as_str).unwrap_or(full_name),
            "url": repo_url,
            "description": repo.get("description").and_then(Value::as_str).unwrap_or(""),
            "defaultBranch": repo.get("default_branch").and_then(Value::as_str).unwrap_or(""),
            "stars": repo.get("stargazers_count").and_then(Value::as_u64).unwrap_or(0),
            "forks": repo.get("forks_count").and_then(Value::as_u64).unwrap_or(0),
            "watchers": repo.get("subscribers_count").or_else(|| repo.get("watchers_count")).and_then(Value::as_u64).unwrap_or(0),
            "openIssueTotal": repo.get("open_issues_count").and_then(Value::as_u64).unwrap_or(0),
            "hasPages": repo.get("has_pages").and_then(Value::as_bool).unwrap_or(false),
            "homepage": repo.get("homepage").and_then(Value::as_str).unwrap_or(""),
        },
        "issues": issues,
        "pullRequests": pulls,
        "releases": normalize_releases(releases),
        "pages": pages,
        "deployments": normalize_deployments(deployments, full_name),
        "packages": packages,
        "links": {
            "issues": format!("{repo_url}/issues"),
            "pullRequests": format!("{repo_url}/pulls"),
            "releases": format!("{repo_url}/releases"),
            "newRelease": format!("{repo_url}/releases/new"),
            "deployments": format!("{repo_url}/deployments"),
            "packages": format!("{repo_url}/pkgs"),
            "pagesSettings": format!("{repo_url}/settings/pages"),
        }
    }))
}

fn search_repo_items(account: &str, full_name: &str, kind: &str) -> Value {
    let query = format!("repo:{full_name} type:{kind} state:open");
    let value = gh_api_json(
        account,
        vec![
            "--method".into(),
            "GET".into(),
            "search/issues".into(),
            "-f".into(),
            format!("q={query}"),
            "-F".into(),
            "per_page=5".into(),
        ],
    )
    .unwrap_or_else(|error| json!({ "total_count": 0, "items": [], "error": error.to_string() }));

    let items = value
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            json!({
                "number": item.get("number").and_then(Value::as_u64).unwrap_or(0),
                "title": item.get("title").and_then(Value::as_str).unwrap_or(""),
                "url": item.get("html_url").and_then(Value::as_str).unwrap_or(""),
                "updatedAt": item.get("updated_at").and_then(Value::as_str).unwrap_or(""),
                "state": item.get("state").and_then(Value::as_str).unwrap_or(""),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "count": value.get("total_count").and_then(Value::as_u64).unwrap_or(0),
        "items": items,
        "error": value.get("error").and_then(Value::as_str).unwrap_or(""),
    })
}

fn normalize_releases(value: Value) -> Value {
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return json!({ "count": 0, "items": [], "error": error });
    }
    let items = value
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(5)
        .map(|item| {
            json!({
                "name": item.get("name").and_then(Value::as_str).unwrap_or(""),
                "tagName": item.get("tag_name").and_then(Value::as_str).unwrap_or(""),
                "url": item.get("html_url").and_then(Value::as_str).unwrap_or(""),
                "publishedAt": item.get("published_at").and_then(Value::as_str).unwrap_or(""),
                "draft": item.get("draft").and_then(Value::as_bool).unwrap_or(false),
                "prerelease": item.get("prerelease").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect::<Vec<_>>();
    json!({ "count": items.len(), "items": items })
}

fn normalize_deployments(value: Value, full_name: &str) -> Value {
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return json!({ "count": 0, "items": [], "error": error });
    }
    let items = value
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(5)
        .map(|item| {
            json!({
                "id": item.get("id").and_then(Value::as_u64).unwrap_or(0),
                "environment": item.get("environment").and_then(Value::as_str).unwrap_or(""),
                "createdAt": item.get("created_at").and_then(Value::as_str).unwrap_or(""),
                "updatedAt": item.get("updated_at").and_then(Value::as_str).unwrap_or(""),
                "url": format!("https://github.com/{full_name}/deployments"),
            })
        })
        .collect::<Vec<_>>();
    json!({ "count": items.len(), "items": items })
}

fn repo_packages_graphql(account: &str, owner: &str, name: &str) -> Result<Value> {
    let query = r#"
      query($owner: String!, $name: String!) {
        repository(owner: $owner, name: $name) {
          packages(first: 5) {
            totalCount
            nodes {
              name
              packageType
              latestVersion {
                version
              }
            }
          }
        }
      }
    "#;
    let value = gh_api_json(
        account,
        vec![
            "graphql".into(),
            "-f".into(),
            format!("query={query}"),
            "-F".into(),
            format!("owner={owner}"),
            "-F".into(),
            format!("name={name}"),
        ],
    )?;
    let packages = value
        .get("data")
        .and_then(|data| data.get("repository"))
        .and_then(|repo| repo.get("packages"))
        .cloned()
        .unwrap_or_else(|| json!({ "totalCount": 0, "nodes": [] }));
    let items = packages
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|item| {
            json!({
                "name": item.get("name").and_then(Value::as_str).unwrap_or(""),
                "type": item.get("packageType").and_then(Value::as_str).unwrap_or(""),
                "url": "",
                "latestVersion": item.get("latestVersion").and_then(|version| version.get("version")).and_then(Value::as_str).unwrap_or(""),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "count": packages.get("totalCount").and_then(Value::as_u64).unwrap_or(0),
        "items": items,
    }))
}

fn gh_api_json(account: &str, args: Vec<String>) -> Result<Value> {
    let mut full_args = vec!["api".to_string()];
    full_args.extend(args);
    let refs = full_args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = gh(account, &refs)?;
    serde_json::from_slice(&output.stdout).context("parse gh api json")
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
    let scan_roots = request_scan_roots(&payload);

    let cached_inventory = read_inventory(&state.inventory_path).ok();
    match scan_inventory(
        &accounts,
        &scan_roots,
        max_depth,
        fetch,
        cached_inventory.as_ref(),
    )
    .and_then(|inventory| {
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

fn request_scan_roots(payload: &Value) -> Vec<PathBuf> {
    let requested = payload
        .get("scanRoots")
        .and_then(Value::as_array)
        .map(|roots| {
            roots
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    merge_scan_roots(requested, default_scan_roots())
}

fn merge_scan_roots(primary: Vec<PathBuf>, fallback: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut roots = Vec::new();
    for root in primary.into_iter().chain(fallback) {
        let key = normalize_path_key(&root);
        if seen.insert(key) {
            roots.push(root);
        }
    }
    roots
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
    let local_projects_in = inventory
        .get("localProjects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut rows = Vec::new();
    let mut status_counts = BTreeMap::<String, u64>::new();
    let mut language_counts = BTreeMap::<String, u64>::new();
    let mut category_counts = BTreeMap::<String, u64>::new();
    let mut context_kind_counts = BTreeMap::<String, u64>::new();
    let mut public_count = 0_u64;
    let mut private_count = 0_u64;
    let mut fork_count = 0_u64;
    let mut archived_count = 0_u64;
    let mut local_match_count = 0_u64;
    let mut local_contexts_by_remote = BTreeMap::<String, Vec<Value>>::new();
    for project in &local_projects_in {
        for key in value_string_array(project.get("remoteKeys")) {
            local_contexts_by_remote
                .entry(key.to_ascii_lowercase())
                .or_default()
                .push(project.clone());
        }
    }
    let scanned_remote_keys = rows_in
        .iter()
        .filter_map(|row| {
            row.get("remote")
                .and_then(|remote| remote.get("repoKey"))
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
        })
        .collect::<BTreeSet<_>>();
    let unlinked_context_count = local_projects_in
        .iter()
        .filter(|project| context_project_unlinked(project, &scanned_remote_keys))
        .count();

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

        let repo_key = remote.get("repoKey").and_then(Value::as_str).unwrap_or("");
        let name = remote
            .get("nameWithOwner")
            .and_then(Value::as_str)
            .unwrap_or("");
        let description = remote
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let topics = repo_topics(&remote);
        let local_context_matches = row
            .get("localContextMatches")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| {
                local_contexts_by_remote
                    .get(repo_key)
                    .cloned()
                    .unwrap_or_default()
            });
        let classification = classify_repo_context(
            name,
            description,
            &topics,
            &local_path_text,
            &local_context_matches,
        );
        let categories = classification.categories;
        let category = primary_category(&categories);
        let primary_label = category_label(&category);
        let category_labels = categories
            .iter()
            .map(|category| Value::from(category_label(category)))
            .collect::<Vec<_>>();
        for category in &categories {
            *category_counts.entry(category.clone()).or_insert(0) += 1;
            *context_kind_counts.entry(category.clone()).or_insert(0) += 1;
        }
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
            "accountAlias": remote.get("accountAlias").and_then(Value::as_str).unwrap_or(""),
            "accountLogin": remote.get("accountLogin").and_then(Value::as_str).unwrap_or(""),
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
            "categoryLabel": primary_label,
            "categories": categories.clone(),
            "categoryLabels": category_labels.clone(),
            "contextKinds": categories,
            "contextLabels": category_labels,
            "contextEvidence": classification.evidence,
            "localContextMatches": local_context_matches,
            "topics": topics,
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
            let categories = classify_repo_categories(&remote_text, path, &local_context);
            let category = primary_category(&categories);
            let primary_label = category_label(&category);
            let category_labels = categories
                .iter()
                .map(|category| Value::from(category_label(category)))
                .collect::<Vec<_>>();
            json!({
                "id": format!("local-only-{index}"),
                "path": path,
                "branch": local.get("branch").and_then(Value::as_str).unwrap_or(""),
                "status": local.get("status").and_then(Value::as_str).unwrap_or("unknown"),
                "category": category,
                "categoryLabel": primary_label,
                "categories": categories.clone(),
                "categoryLabels": category_labels.clone(),
                "contextKinds": categories,
                "contextLabels": category_labels,
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

    let local_projects = local_projects_in
        .iter()
        .enumerate()
        .map(|(index, project)| {
            let path = project.get("path").and_then(Value::as_str).unwrap_or("");
            let categories = project
                .get("categories")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .filter(|items| !items.is_empty())
                .unwrap_or_else(|| classify_repo_categories(
                    project.get("name").and_then(Value::as_str).unwrap_or(""),
                    "",
                    &[path.to_string()],
                ));
            let category = primary_category(&categories);
            let primary_label = category_label(&category);
            let category_labels = categories
                .iter()
                .map(|category| Value::from(category_label(category)))
                .collect::<Vec<_>>();
            json!({
                "id": project.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("local-project-{index}")),
                "name": project.get("name").and_then(Value::as_str).unwrap_or(""),
                "path": path,
                "category": category,
                "categoryLabel": primary_label,
                "categories": categories.clone(),
                "categoryLabels": category_labels.clone(),
                "contextKinds": categories,
                "contextLabels": category_labels,
                "evidence": project.get("evidence").cloned().unwrap_or_else(|| json!([])),
                "gitScope": project.get("gitScope").and_then(Value::as_str).unwrap_or("none"),
                "isGitRepo": project.get("isGitRepo").and_then(Value::as_bool).unwrap_or(false),
                "nearestGitRoot": project.get("nearestGitRoot").and_then(Value::as_str).unwrap_or(""),
                "gitStatus": project.get("gitStatus").and_then(Value::as_str).unwrap_or("not-git"),
                "branch": project.get("branch").and_then(Value::as_str).unwrap_or(""),
                "dirty": project.get("dirty").and_then(Value::as_bool).unwrap_or(false),
                "remotes": project.get("remotes").cloned().unwrap_or_else(|| json!([])),
                "remoteKeys": project.get("remoteKeys").cloned().unwrap_or_else(|| json!([])),
                "modifiedAt": project.get("modifiedAt").and_then(Value::as_str).unwrap_or(""),
                "raw": project,
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
            "localProjectCount": local_projects.len(),
            "localProjectGitCount": local_projects.iter().filter(|project| project.get("isGitRepo").and_then(Value::as_bool).unwrap_or(false)).count(),
            "localProjectNoGitCount": local_projects.iter().filter(|project| !project.get("isGitRepo").and_then(Value::as_bool).unwrap_or(false)).count(),
            "unlinkedContextCount": unlinked_context_count,
            "localMatchCount": local_match_count,
            "publicCount": public_count,
            "privateCount": private_count,
            "forkCount": fork_count,
            "archivedCount": archived_count,
            "statusCounts": status_counts,
            "languageCounts": language_counts,
            "categoryCounts": category_counts,
            "contextKindCounts": context_kind_counts,
            "scanRoots": inventory.get("scanRoots").cloned().unwrap_or_else(|| json!([])),
            "accounts": inventory.get("accounts").cloned().unwrap_or_else(|| legacy_accounts(inventory)),
            "accountErrors": inventory.get("accountErrors").cloned().unwrap_or_else(|| json!([])),
            "accountAlias": inventory.get("accountAlias").and_then(Value::as_str).unwrap_or(""),
            "accountLogin": inventory.get("accountLogin").and_then(Value::as_str).unwrap_or(""),
            "versionCheckUsedFetch": inventory.get("versionCheckUsedFetch").cloned().unwrap_or(Value::Null),
        },
        "rows": rows,
        "localOnly": local_only,
        "localProjects": local_projects,
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
    primary_category(&classify_repo_categories(name, description, local_paths))
}

fn classify_repo_categories(name: &str, description: &str, local_paths: &[String]) -> Vec<String> {
    classify_repo_context(name, description, &[], local_paths, &[]).categories
}

#[derive(Default)]
struct ContextClassification {
    categories: Vec<String>,
    evidence: Vec<String>,
}

fn classify_repo_context(
    name: &str,
    description: &str,
    topics: &[String],
    local_paths: &[String],
    local_contexts: &[Value],
) -> ContextClassification {
    let name_paths_topics =
        format!("{} {} {}", name, topics.join(" "), local_paths.join(" ")).to_ascii_lowercase();
    let description_lower = description.to_ascii_lowercase();
    let haystack = format!("{name_paths_topics} {description_lower}");
    let mut categories = Vec::new();
    let mut evidence = Vec::new();

    if contains_any_token(
        &haystack,
        &[
            "agent",
            "agents",
            "agentic",
            "codex",
            "claude",
            "autogen",
            "langgraph",
            "crew",
        ],
    ) || contains_any(
        &haystack,
        &[
            "agent",
            "codex",
            "claude",
            ".codex",
            ".claude",
            ".agents",
            "agents.md",
            "claude.md",
        ],
    ) {
        push_context(
            &mut categories,
            &mut evidence,
            "agents",
            "repo text: agent/codex/claude",
        );
    }

    if contains_any_token(
        &haystack,
        &[
            "memory",
            "memories",
            "knowledge",
            "rag",
            "vector",
            "obsidian",
        ],
    ) || contains_any(
        &haystack,
        &[
            "memory",
            "memories",
            "knowledge",
            "obsidian",
            "memory-bank",
            "memory.md",
        ],
    ) {
        push_context(
            &mut categories,
            &mut evidence,
            "memory",
            "repo text: memory/knowledge",
        );
    }

    if contains_any_token(&name_paths_topics, &["skill", "skills"])
        || contains_any(
            &name_paths_topics,
            &[
                "skill",
                "skills",
                "skill.md",
                ".codex/skills",
                ".codex\\skills",
            ],
        )
        || contains_any(
            &description_lower,
            &[
                "codex skill",
                "codex skills",
                "agent skill pack",
                "skill pack",
                "skill repository",
            ],
        )
    {
        push_context(
            &mut categories,
            &mut evidence,
            "skills",
            "repo text: skill marker",
        );
    }

    if contains_any_token(&haystack, &["mcp"])
        || contains_any(
            &haystack,
            &[
                "mcp",
                "model-context-protocol",
                ".mcp.json",
                "mcp.json",
                "mcp-server",
            ],
        )
    {
        push_context(
            &mut categories,
            &mut evidence,
            "mcp",
            "repo text: mcp marker",
        );
    }

    if contains_any_token(&haystack, &["workflow", "workflows", "actions"])
        || contains_any(
            &haystack,
            &[
                "workflow",
                "workflows",
                ".github/workflows",
                ".github\\workflows",
                "github actions",
                "ci/cd",
            ],
        )
    {
        push_context(
            &mut categories,
            &mut evidence,
            "workflow",
            "repo text: workflow/actions",
        );
    }

    if contains_any_token(&haystack, &["rule", "rules"])
        || contains_any(
            &haystack,
            &[
                "rule",
                "rules",
                ".cursor/rules",
                ".cursor\\rules",
                ".cursorrules",
                "rules.md",
                "agents.md",
                "claude.md",
            ],
        )
    {
        push_context(
            &mut categories,
            &mut evidence,
            "rules",
            "repo text: rules marker",
        );
    }

    if contains_any_token(&haystack, &["hook", "hooks", "webhook"])
        || contains_any(
            &haystack,
            &[
                "hook",
                "hooks",
                ".githooks",
                "git-hook",
                "githook",
                "pre-commit",
                "pre-push",
            ],
        )
    {
        push_context(
            &mut categories,
            &mut evidence,
            "hook",
            "repo text: hook marker",
        );
    }

    for context in local_contexts {
        for category in value_string_array(context.get("categories")) {
            let context_evidence = value_string_array(context.get("evidence"));
            let label = if context_evidence.is_empty() {
                "local context marker".to_string()
            } else {
                context_evidence.join(", ")
            };
            push_context(&mut categories, &mut evidence, &category, &label);
        }
    }

    if categories.is_empty() {
        categories.push("other".to_string());
    }

    ContextClassification {
        categories: order_context_categories(unique_owned_strings(categories)),
        evidence: unique_owned_strings(evidence),
    }
}

fn push_context(
    categories: &mut Vec<String>,
    evidence: &mut Vec<String>,
    category: &str,
    evidence_text: &str,
) {
    categories.push(category.to_string());
    if !evidence_text.is_empty() {
        evidence.push(evidence_text.to_string());
    }
}

fn order_context_categories(categories: Vec<String>) -> Vec<String> {
    let values = categories.into_iter().collect::<BTreeSet<_>>();
    let mut ordered = [
        "agents", "memory", "skills", "mcp", "workflow", "rules", "hook", "other",
    ]
    .iter()
    .filter(|category| values.iter().any(|value| value == *category))
    .map(|category| (*category).to_string())
    .collect::<Vec<_>>();
    for category in values {
        if !ordered.contains(&category) {
            ordered.push(category);
        }
    }
    ordered
}

fn value_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn repo_topics(remote: &Value) -> Vec<String> {
    let mut topics = Vec::new();
    if let Some(items) = remote.get("topics").and_then(Value::as_array) {
        topics.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
    }
    if let Some(items) = remote.get("repositoryTopics").and_then(Value::as_array) {
        for item in items {
            if let Some(name) = item
                .get("topic")
                .and_then(|topic| topic.get("name"))
                .and_then(Value::as_str)
                .or_else(|| item.get("name").and_then(Value::as_str))
            {
                topics.push(name.to_string());
            }
        }
    }
    unique_owned_strings(topics)
}

fn context_project_unlinked(project: &Value, remote_keys: &BTreeSet<String>) -> bool {
    let keys = value_string_array(project.get("remoteKeys"));
    keys.is_empty()
        || !keys
            .iter()
            .map(|key| key.to_ascii_lowercase())
            .any(|key| remote_keys.contains(&key))
}

fn unique_owned_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn primary_category(categories: &[String]) -> String {
    categories
        .first()
        .cloned()
        .unwrap_or_else(|| "other".to_string())
}

fn category_label(category: &str) -> &'static str {
    match category {
        "agents" => "Agents",
        "memory" => "Memory",
        "skills" => "Skills",
        "mcp" => "MCP",
        "workflow" => "Workflow",
        "rules" => "Rules",
        "hook" => "Hooks",
        _ => "Other",
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn contains_any_token(haystack: &str, needles: &[&str]) -> bool {
    haystack
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| needles.contains(&token))
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
    if let Some(projects) = inventory.get("localProjects").and_then(Value::as_array) {
        for project in projects {
            if let Some(path) = project.get("path").and_then(Value::as_str) {
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
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    roots.insert(current_dir.clone());
    for ancestor in current_dir
        .ancestors()
        .take(4)
        .filter(|ancestor| ancestor.parent().is_some())
    {
        roots.insert(ancestor.to_path_buf());
    }
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
    for candidate in [
        PathBuf::from(r"D:\study\code"),
        PathBuf::from(r"D:\study\code\0ai\产品"),
        PathBuf::from(r"D:\study\code\0ai\产品\output\项目目录"),
    ] {
        if candidate.exists() {
            roots.insert(candidate);
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

fn worker_limit(env_name: &str, default: usize) -> usize {
    let requested = env::var(env_name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default);
    let available = thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(default.max(1));
    requested.min(available.max(1)).max(1)
}

fn remote_worker_limit() -> usize {
    worker_limit("REPO_ATLAS_REMOTE_WORKERS", DEFAULT_REMOTE_WORKERS)
}

fn local_scan_worker_limit() -> usize {
    worker_limit("REPO_ATLAS_LOCAL_SCAN_WORKERS", DEFAULT_LOCAL_SCAN_WORKERS)
}

fn local_git_worker_limit(fetch: bool) -> usize {
    let default = if fetch {
        DEFAULT_LOCAL_GIT_WORKERS.min(4)
    } else {
        DEFAULT_LOCAL_GIT_WORKERS
    };
    worker_limit("REPO_ATLAS_LOCAL_GIT_WORKERS", default)
}

fn parallel_map_limited<T, U, F>(items: Vec<T>, limit: usize, mapper: F) -> Vec<U>
where
    T: Send,
    U: Send,
    F: Fn(T) -> U + Sync,
{
    let mut results = Vec::with_capacity(items.len());
    let mut items = items.into_iter();
    let limit = limit.max(1);
    loop {
        let batch = items.by_ref().take(limit).collect::<Vec<_>>();
        if batch.is_empty() {
            break;
        }
        let batch_results = thread::scope(|scope| {
            let handles = batch
                .into_iter()
                .map(|item| {
                    let mapper = &mapper;
                    scope.spawn(move || mapper(item))
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| match handle.join() {
                    Ok(value) => value,
                    Err(_) => panic!("parallel scan worker panicked"),
                })
                .collect::<Vec<_>>()
        });
        results.extend(batch_results);
    }
    results
}

fn scan_inventory(
    accounts: &[String],
    scan_roots: &[PathBuf],
    max_depth: usize,
    fetch: bool,
    cached_inventory: Option<&Value>,
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
    let mut remote_source = "live";

    for (account, result) in parallel_map_limited(
        requested_accounts,
        remote_worker_limit(),
        |account: String| {
            let result = list_remote_repos(&account).map_err(|error| error.to_string());
            (account, result)
        },
    ) {
        match result {
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
                    "error": error,
                }));
            }
        }
    }

    if remote_repos.is_empty() && !account_errors.is_empty() {
        remote_repos = cached_remote_repos(cached_inventory);
        if remote_repos.is_empty() {
            return Err(anyhow!(
                "No GitHub repositories could be loaded. {}",
                account_errors
                    .iter()
                    .filter_map(|item| item.get("error").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        remote_source = "cached";
        if account_summaries.is_empty() {
            account_summaries = cached_accounts(cached_inventory);
        }
        account_errors.push(json!({
            "alias": "cache",
            "error": "Using the last saved remote repository list because live GitHub loading failed.",
        }));
    }
    remote_repos.sort_by(|a, b| {
        a.get("nameWithOwner")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("nameWithOwner").and_then(Value::as_str).unwrap_or(""))
    });

    let local_scan = scan_local_filesystem(scan_roots, max_depth);
    let local_repos = inspect_local_repos(local_scan.git_roots, fetch);
    let local_projects = build_context_projects(local_scan.contexts, &local_repos);
    let mut inventory = merge_inventory(remote_repos, local_repos, local_projects);
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
    inventory
        .as_object_mut()
        .unwrap()
        .insert("remoteSource".into(), Value::from(remote_source));
    Ok(inventory)
}

fn cached_remote_repos(cached_inventory: Option<&Value>) -> Vec<Value> {
    cached_inventory
        .and_then(|inventory| inventory.get("rows").and_then(Value::as_array))
        .map(|rows| {
            let mut seen = BTreeSet::new();
            rows.iter()
                .filter_map(|row| row.get("remote").cloned())
                .filter(|repo| {
                    repo.get("repoKey")
                        .and_then(Value::as_str)
                        .map(|key| seen.insert(key.to_string()))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn cached_accounts(cached_inventory: Option<&Value>) -> Vec<Value> {
    cached_inventory
        .and_then(|inventory| inventory.get("accounts").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default()
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
    let fields = "nameWithOwner,url,description,isPrivate,isArchived,isFork,primaryLanguage,pushedAt,updatedAt,defaultBranchRef,repositoryTopics";
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
    let jq = ".[] | {nameWithOwner:.full_name,url:.html_url,description:.description,isPrivate:.private,isArchived:.archived,isFork:.fork,primaryLanguage:(if .language == null then null else {name:.language} end),pushedAt:.pushed_at,updatedAt:.updated_at,defaultBranchRef:{name:.default_branch},repositoryTopics:(if .topics == null then [] else [.topics[] | {topic:{name:.}}] end)}";
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

#[derive(Clone)]
struct LocalContextCandidate {
    path: PathBuf,
    signals: LocalContextSignals,
}

#[derive(Default)]
struct LocalScanIndex {
    git_roots: Vec<PathBuf>,
    contexts: Vec<LocalContextCandidate>,
}

fn scan_local_filesystem(scan_roots: &[PathBuf], max_depth: usize) -> LocalScanIndex {
    let roots = scan_roots
        .iter()
        .filter(|root| root.exists())
        .cloned()
        .collect::<Vec<_>>();
    let parts = parallel_map_limited(roots, local_scan_worker_limit(), |root| {
        scan_local_root(&root, max_depth)
    });
    merge_local_scan_indexes(parts)
}

fn scan_local_root(root: &Path, max_depth: usize) -> LocalScanIndex {
    let mut git_roots = BTreeSet::new();
    let mut contexts = BTreeMap::<String, LocalContextCandidate>::new();
    let walker = WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|entry| !should_skip_entry(entry));
    for entry in walker.flatten().filter(|entry| entry.file_type().is_dir()) {
        let path = entry.path();
        if path.join(".git").exists() {
            let top = git_output(path, &["rev-parse", "--show-toplevel"])
                .map(PathBuf::from)
                .unwrap_or_else(|| path.to_path_buf());
            git_roots.insert(top);
        }

        let signals = local_context_signals(path);
        if !signals.categories.is_empty() {
            let key = normalize_path_key(path);
            contexts
                .entry(key)
                .or_insert_with(|| LocalContextCandidate {
                    path: path.to_path_buf(),
                    signals,
                });
        }
    }
    LocalScanIndex {
        git_roots: git_roots.into_iter().collect(),
        contexts: contexts.into_values().collect(),
    }
}

fn merge_local_scan_indexes(parts: Vec<LocalScanIndex>) -> LocalScanIndex {
    let mut git_roots = BTreeSet::new();
    let mut contexts = BTreeMap::<String, LocalContextCandidate>::new();
    for part in parts {
        for path in part.git_roots {
            git_roots.insert(path);
        }
        for candidate in part.contexts {
            contexts
                .entry(normalize_path_key(&candidate.path))
                .or_insert(candidate);
        }
    }
    LocalScanIndex {
        git_roots: git_roots.into_iter().collect(),
        contexts: contexts.into_values().collect(),
    }
}

fn find_git_roots(scan_roots: &[PathBuf], max_depth: usize) -> Vec<PathBuf> {
    scan_local_filesystem(scan_roots, max_depth).git_roots
}

fn inspect_local_repos(git_roots: Vec<PathBuf>, fetch: bool) -> Vec<Value> {
    parallel_map_limited(git_roots, local_git_worker_limit(fetch), move |path| {
        inspect_local_repo(&path, fetch)
    })
}

fn find_context_projects(
    scan_roots: &[PathBuf],
    max_depth: usize,
    local_repos: &[Value],
) -> Vec<Value> {
    let local_scan = scan_local_filesystem(scan_roots, max_depth);
    build_context_projects(local_scan.contexts, local_repos)
}

fn build_context_projects(
    contexts: Vec<LocalContextCandidate>,
    local_repos: &[Value],
) -> Vec<Value> {
    let mut local_git_roots = Vec::<(String, Value)>::new();
    for local in local_repos {
        if let Some(path) = local.get("path").and_then(Value::as_str) {
            local_git_roots.push((normalize_path_key(Path::new(path)), local.clone()));
        }
    }

    let mut projects = BTreeMap::<String, Value>::new();
    for candidate in contexts {
        let path = candidate.path;
        let signals = candidate.signals;
        let key = normalize_path_key(&path);
        if projects.contains_key(&key) {
            continue;
        }
        let local_git = nearest_local_repo(&key, &local_git_roots);
        let nearest_git_root = local_git
            .and_then(|(_, local)| local.get("path").and_then(Value::as_str))
            .unwrap_or("");
        let git_scope = match local_git {
            Some((root_key, _)) if root_key == &key => "self",
            Some(_) => "inside",
            None => "none",
        };
        let is_git_repo = git_scope == "self";
        let remotes = local_git
            .and_then(|(_, local)| local.get("remotes"))
            .cloned()
            .unwrap_or_else(|| json!([]));
        let remote_keys = remotes
            .as_array()
            .map(|items| {
                Value::Array(
                    items
                        .iter()
                        .filter_map(|remote| remote.get("repoKey").and_then(Value::as_str))
                        .map(Value::from)
                        .collect(),
                )
            })
            .unwrap_or_else(|| json!([]));
        let categories = signals.categories.clone();
        let context_labels = signals.kinds.clone();
        let evidence = signals.evidence.clone();

        projects.insert(
            key.clone(),
            json!({
                "id": format!("local-project-{key}"),
                "name": path.file_name().and_then(|value| value.to_str()).unwrap_or("local-project"),
                "path": path.to_string_lossy().to_string(),
                "categories": categories.clone(),
                "contextKinds": categories,
                "contextLabels": context_labels,
                "evidence": evidence,
                "gitScope": git_scope,
                "isGitRepo": is_git_repo,
                "nearestGitRoot": nearest_git_root,
                "gitStatus": local_git.and_then(|(_, local)| local.get("status").and_then(Value::as_str)).unwrap_or("not-git"),
                "branch": local_git.and_then(|(_, local)| local.get("branch").and_then(Value::as_str)).unwrap_or(""),
                "dirty": local_git.and_then(|(_, local)| local.get("dirty").and_then(Value::as_bool)).unwrap_or(false),
                "remotes": remotes,
                "remoteKeys": remote_keys,
                "modifiedAt": path_modified_at(&path),
            }),
        );
    }
    projects.into_values().collect()
}

#[derive(Clone)]
struct LocalContextSignals {
    categories: Vec<String>,
    kinds: Vec<Value>,
    evidence: Vec<Value>,
}

fn local_context_signals(path: &Path) -> LocalContextSignals {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let path_text = path.to_string_lossy().to_ascii_lowercase();
    let mut categories = Vec::new();
    let mut kinds = Vec::new();
    let mut evidence = Vec::new();

    let has_skill_marker = path.join("SKILL.md").is_file()
        || path.join("skill.json").is_file()
        || path.join("skills").exists()
        || path.join(".codex").join("skills").exists()
        || contains_any(&path_text, &[".codex/skills", ".codex\\skills"])
        || contains_any(&file_name, &["codex-skill", "agent-skill"])
        || file_name.contains("skill");
    if has_skill_marker {
        categories.push("skills".to_string());
        kinds.push(Value::from("Skills"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[
                "SKILL.md",
                "skill.json",
                "skills",
                ".codex/skills",
                "directory name: skill",
            ],
        );
    }

    let has_mcp_marker = path.join("mcp.json").is_file()
        || path.join(".mcp.json").is_file()
        || contains_any(
            &file_name,
            &["mcp-server", "model-context-protocol", "context-mcp"],
        )
        || file_name.contains("mcp");
    if has_mcp_marker {
        categories.push("mcp".to_string());
        kinds.push(Value::from("MCP"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[".mcp.json", "mcp.json", "mcp", "directory name: mcp"],
        );
    }

    let has_hook_marker = path.join(".githooks").exists()
        || path.join("hooks").exists()
        || path.join(".pre-commit-config.yaml").is_file()
        || contains_any(
            &file_name,
            &["git-hook", "githook", "pre-commit", "pre-push", "webhook"],
        )
        || file_name.contains("hook");
    if has_hook_marker {
        categories.push("hook".to_string());
        kinds.push(Value::from("Hooks"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[
                ".githooks",
                "hooks",
                ".pre-commit-config.yaml",
                "directory name: hook",
            ],
        );
    }

    let has_agent_marker = path.join("AGENTS.md").is_file()
        || path.join("CLAUDE.md").is_file()
        || path.join(".codex").exists()
        || path.join(".claude").exists()
        || path.join(".agents").exists()
        || path.join("agents").exists()
        || file_name == ".codex"
        || file_name == ".claude"
        || file_name == ".agents"
        || contains_any(&file_name, &["agent-"])
        || file_name.contains("codex")
        || file_name.contains("claude")
        || file_name.contains("agent");
    if has_agent_marker {
        categories.push("agents".to_string());
        kinds.push(Value::from("Agents"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[
                "AGENTS.md",
                "CLAUDE.md",
                ".codex",
                ".claude",
                ".agents",
                "agents",
                "directory name: agent/codex/claude",
            ],
        );
    }

    let has_memory_marker = path.join("MEMORY.md").is_file()
        || path.join("memory").exists()
        || path.join("memories").exists()
        || path.join("memory-bank").exists()
        || path.join("knowledge").exists()
        || contains_any(
            &file_name,
            &["memory", "memories", "memory-bank", "knowledge"],
        );
    if has_memory_marker {
        categories.push("memory".to_string());
        kinds.push(Value::from("Memory"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[
                "MEMORY.md",
                "memory",
                "memories",
                "memory-bank",
                "knowledge",
                "directory name: memory/knowledge",
            ],
        );
    }

    let has_workflow_marker = path.join(".github").join("workflows").exists()
        || path.join("workflows").exists()
        || path.join("workflow").exists()
        || contains_any(&path_text, &[".github/workflows", ".github\\workflows"])
        || contains_any(&file_name, &["workflow", "workflows"]);
    if has_workflow_marker {
        categories.push("workflow".to_string());
        kinds.push(Value::from("Workflow"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[
                ".github/workflows",
                "workflows",
                "workflow",
                "directory name: workflow",
            ],
        );
    }

    let has_rules_marker = path.join(".cursor").join("rules").exists()
        || path.join(".cursorrules").is_file()
        || path.join("rules").exists()
        || path.join("RULES.md").is_file()
        || path.join("AGENTS.md").is_file()
        || path.join("CLAUDE.md").is_file()
        || contains_any(&path_text, &[".cursor/rules", ".cursor\\rules"])
        || file_name.contains("rules");
    if has_rules_marker {
        categories.push("rules".to_string());
        kinds.push(Value::from("Rules"));
        push_marker_evidence(
            path,
            &mut evidence,
            &[
                ".cursor/rules",
                ".cursorrules",
                "rules",
                "RULES.md",
                "AGENTS.md",
                "CLAUDE.md",
                "directory name: rules",
            ],
        );
    }

    LocalContextSignals {
        categories: order_context_categories(unique_owned_strings(categories)),
        kinds: unique_values(kinds),
        evidence: unique_values(evidence),
    }
}

fn nearest_local_repo<'a>(
    path_key: &str,
    local_git_roots: &'a [(String, Value)],
) -> Option<&'a (String, Value)> {
    local_git_roots
        .iter()
        .filter(|(root_key, _)| {
            path_key == root_key
                || path_key.starts_with(&format!("{root_key}\\"))
                || path_key.starts_with(&format!("{root_key}/"))
        })
        .max_by_key(|(root_key, _)| root_key.len())
}

fn push_marker_evidence(path: &Path, evidence: &mut Vec<Value>, markers: &[&str]) {
    for marker in markers {
        if marker.starts_with("directory name:") {
            continue;
        }
        let marker_path = marker.replace('/', std::path::MAIN_SEPARATOR_STR);
        if path.join(&marker_path).exists() {
            evidence.push(Value::from(*marker));
        }
    }
}

fn unique_values(values: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.to_string()))
        .collect()
}

fn path_modified_at(path: &Path) -> String {
    let Ok(metadata) = fs::metadata(path) else {
        return String::new();
    };
    let Ok(modified) = metadata.modified() else {
        return String::new();
    };
    let datetime: chrono::DateTime<chrono::Utc> = modified.into();
    datetime.to_rfc3339()
}

fn should_skip_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    matches!(
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
    ) || name.starts_with(".cache")
}

fn inspect_local_repo(repo_path: &Path, fetch: bool) -> Value {
    let mut status = "unknown".to_string();
    let mut error = Value::Null;

    if fetch {
        let _ = run_git(&["fetch", "--all", "--prune", "--quiet"], Some(repo_path));
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
    if let Some(text) = git_output(repo_path, &["remote", "-v"]) {
        parse_git_remote_output(&text, &mut seen, &mut remotes);
    }
    if remotes.is_empty() {
        parse_git_config_remotes(repo_path, &mut seen, &mut remotes);
    }
    remotes
}

fn parse_git_remote_output(text: &str, seen: &mut BTreeSet<String>, remotes: &mut Vec<Value>) {
    for line in text.lines().filter(|line| line.contains("(fetch)")) {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            continue;
        }
        push_remote(remotes, seen, parts[0], parts[1]);
    }
}

fn parse_git_config_remotes(
    repo_path: &Path,
    seen: &mut BTreeSet<String>,
    remotes: &mut Vec<Value>,
) {
    let Some(git_dir) = git_metadata_dir(repo_path) else {
        return;
    };
    let Ok(text) = fs::read_to_string(git_dir.join("config")) else {
        return;
    };
    let mut current_remote = None::<String>;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            current_remote = parse_remote_section(line);
            continue;
        }
        let Some(remote_name) = current_remote.as_deref() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("url") {
            push_remote(remotes, seen, remote_name, value.trim());
        }
    }
}

fn parse_remote_section(line: &str) -> Option<String> {
    let remote = line.strip_prefix("[remote ")?.strip_suffix(']')?.trim();
    Some(remote.strip_prefix('"')?.strip_suffix('"')?.to_string())
}

fn push_remote(remotes: &mut Vec<Value>, seen: &mut BTreeSet<String>, name: &str, url: &str) {
    let key = format!("{name} {url}");
    if !seen.insert(key) {
        return;
    }
    remotes.push(json!({
        "name": name,
        "url": url,
        "repoKey": normalize_repo_key(url),
    }));
}

fn git_metadata_dir(repo_path: &Path) -> Option<PathBuf> {
    let dot_git = repo_path.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if !dot_git.is_file() {
        return None;
    }
    let text = fs::read_to_string(dot_git).ok()?;
    let gitdir = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))?
        .trim();
    let path = PathBuf::from(gitdir);
    if path.is_absolute() {
        Some(path)
    } else {
        Some(repo_path.join(path))
    }
}

fn git_output(repo_path: &Path, args: &[&str]) -> Option<String> {
    let output = run_git(args, Some(repo_path));
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

fn run_git(args: &[&str], cwd: Option<&Path>) -> ProcOutput {
    let command = git_command();
    run_path(&command, args, cwd)
}

fn git_command() -> PathBuf {
    GIT_COMMAND.get_or_init(resolve_git_command).clone()
}

fn resolve_git_command() -> PathBuf {
    if let Some(path) = env::var_os("REPO_ATLAS_GIT").map(PathBuf::from) {
        if command_works(&path, &["--version"]) {
            return path;
        }
    }
    for candidate in git_command_candidates() {
        if command_works(&candidate, &["--version"]) {
            return candidate;
        }
    }
    PathBuf::from("git")
}

fn git_command_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("git")];
    if cfg!(windows) {
        for env_name in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = env::var_os(env_name).map(PathBuf::from) {
                candidates.push(base.join("Git").join("cmd").join("git.exe"));
                candidates.push(base.join("Git").join("bin").join("git.exe"));
                candidates.push(
                    base.join("Programs")
                        .join("Git")
                        .join("cmd")
                        .join("git.exe"),
                );
            }
        }
        candidates.push(PathBuf::from(r"C:\Program Files\Git\cmd\git.exe"));
        candidates.push(PathBuf::from(r"C:\Program Files\Git\bin\git.exe"));
        candidates.push(PathBuf::from(r"C:\Program Files (x86)\Git\cmd\git.exe"));
    }

    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|path| seen.insert(normalize_path_key(path)))
        .collect()
}

fn command_works(command: &Path, args: &[&str]) -> bool {
    let mut cmd = Command::new(command);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    configure_command(&mut cmd);
    cmd.output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run(command: &str, args: &[&str], cwd: Option<&Path>) -> ProcOutput {
    run_path(Path::new(command), args, cwd)
}

fn run_path(command: &Path, args: &[&str], cwd: Option<&Path>) -> ProcOutput {
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

fn run_with_input(command: &str, args: &[&str], cwd: Option<&Path>, input: &[u8]) -> ProcOutput {
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(error) = stdin.write_all(input) {
            let _ = child.kill();
            let _ = child.wait();
            return ProcOutput {
                status: failed_status(),
                stdout: vec![],
                stderr: error.to_string().into_bytes(),
            };
        }
    }
    match child.wait_with_output() {
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

fn merge_inventory(
    remote_repos: Vec<Value>,
    local_repos: Vec<Value>,
    local_projects: Vec<Value>,
) -> Value {
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
    let mut local_contexts_by_key: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for project in &local_projects {
        for key in value_string_array(project.get("remoteKeys")) {
            local_contexts_by_key
                .entry(key.to_ascii_lowercase())
                .or_default()
                .push(project.clone());
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
            "localContextMatches": local_contexts_by_key.get(key).cloned().unwrap_or_default(),
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
        "localProjects": local_projects,
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
        "Tags",
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
            repo_category_labels(&repo),
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
        format!(
            "Local context projects: {}",
            summary
                .get("localProjectCount")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
        "".into(),
        "| Repository | Account | Tags | Visibility | Fork | Language | Default branch | Local status | Last pushed | Description |".into(),
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
            md_cell(&repo_category_labels(&repo)),
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

fn repo_category_labels(repo: &Value) -> String {
    repo.get("categoryLabels")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            repo.get("categoryLabel")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
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
        let agent_skills = classify_repo_categories("owner/agent-skills", "Codex skill pack", &[]);
        assert_eq!(
            classify_repo("owner/agent-skills", "Codex skill pack", &[]),
            "agents"
        );
        assert!(agent_skills.contains(&"agents".to_string()));
        assert!(agent_skills.contains(&"skills".to_string()));
        assert_eq!(
            classify_repo("owner/local-mcp-server", "Model context protocol", &[]),
            "mcp"
        );
        assert_eq!(
            classify_repo("owner/memory-bank", "RAG knowledge store", &[]),
            "memory"
        );
        assert_eq!(
            classify_repo("owner/action-hooks", "Reusable git hook automation", &[]),
            "hook"
        );
    }

    #[test]
    fn classify_allows_multiple_categories_without_generic_skill_false_positive() {
        let campus = classify_repo_categories(
            "Harzva/CampusAgent-QA",
            "Agentic campus QA system with RAG Wiki memory, and GBrain skills",
            &[],
        );
        assert!(campus.contains(&"agents".to_string()));
        assert!(campus.contains(&"memory".to_string()));
        assert!(!campus.contains(&"skills".to_string()));

        let mcp_memory = classify_repo_categories(
            "owner/local-mcp-server",
            "Model context protocol server with vector memory",
            &[],
        );
        assert!(mcp_memory.contains(&"mcp".to_string()));
        assert!(mcp_memory.contains(&"memory".to_string()));
    }

    #[test]
    fn find_git_roots_walks_normal_project_dirs() {
        let root = temp_test_dir("git-roots");
        let repo = root.join("normal-repo");
        fs::create_dir_all(&repo).expect("create repo dir");
        let init = run("git", &["init"], Some(&repo));
        assert!(
            init.status.success(),
            "git init failed: {}",
            process_message(&init)
        );

        let found = find_git_roots(&[root.clone()], 4);
        let repo_key = normalize_path_key(&repo);
        assert!(
            found
                .iter()
                .any(|path| normalize_path_key(path) == repo_key),
            "normal project directory should be scanned"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn find_git_roots_keeps_dot_git_dirs_when_git_command_fails() {
        let root = temp_test_dir("git-roots-fallback");
        let repo = root.join("manual-repo");
        fs::create_dir_all(repo.join(".git")).expect("create fake git dir");

        let found = find_git_roots(&[root.clone()], 4);
        let repo_key = normalize_path_key(&repo);
        assert!(
            found
                .iter()
                .any(|path| normalize_path_key(path) == repo_key),
            "a .git marker should keep the local repo candidate even when git rev-parse fails"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_remotes_falls_back_to_git_config() {
        let root = temp_test_dir("git-config-remotes");
        let repo = root.join("config-only-repo");
        fs::create_dir_all(repo.join(".git")).expect("create git dir");
        fs::write(
            repo.join(".git").join("config"),
            r#"
[core]
    repositoryformatversion = 0
[remote "origin"]
    url = https://github.com/Harzva/RepoAtlas.git
    fetch = +refs/heads/*:refs/remotes/origin/*
"#,
        )
        .expect("write git config");

        let remotes = parse_remotes(&repo);
        assert!(remotes.iter().any(|remote| {
            remote.get("repoKey").and_then(Value::as_str) == Some("harzva/repoatlas")
        }));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cached_remote_repos_reads_previous_inventory_rows() {
        let cached = json!({
            "rows": [
                { "remote": { "nameWithOwner": "Harzva/RepoAtlas", "repoKey": "harzva/repoatlas" } },
                { "remote": { "nameWithOwner": "Harzva/RepoAtlas", "repoKey": "harzva/repoatlas" } },
                { "remote": { "nameWithOwner": "Harzva/codex-hooks", "repoKey": "harzva/codex-hooks" } }
            ]
        });

        let repos = cached_remote_repos(Some(&cached));
        assert_eq!(repos.len(), 2);
        assert!(repos.iter().any(|repo| {
            repo.get("repoKey").and_then(Value::as_str) == Some("harzva/repoatlas")
        }));
    }

    #[test]
    fn context_projects_include_non_git_hooks() {
        let root = temp_test_dir("context-projects");
        let hook_project = root.join("custom-hook-pack");
        fs::create_dir_all(hook_project.join("hooks")).expect("create hook project");

        let projects = find_context_projects(&[root.clone()], 4, &[]);
        let project = projects
            .iter()
            .find(|item| {
                item.get("path")
                    .and_then(Value::as_str)
                    .map(|path| {
                        normalize_path_key(Path::new(path)) == normalize_path_key(&hook_project)
                    })
                    .unwrap_or(false)
            })
            .expect("hook context project");
        assert!(!project
            .get("isGitRepo")
            .and_then(Value::as_bool)
            .unwrap_or(true));
        assert!(project
            .get("categories")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("hook")));
        assert_eq!(
            project.get("gitScope").and_then(Value::as_str),
            Some("none")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_context_markers_cover_agent_taxonomy() {
        let root = temp_test_dir("context-markers");
        fs::write(root.join("AGENTS.md"), "").expect("write agents marker");
        fs::create_dir_all(root.join(".codex").join("skills")).expect("create codex skills");
        fs::write(root.join(".mcp.json"), "{}").expect("write mcp marker");
        fs::create_dir_all(root.join(".github").join("workflows")).expect("create workflows");
        fs::create_dir_all(root.join(".githooks")).expect("create githooks");
        fs::write(root.join(".pre-commit-config.yaml"), "").expect("write precommit");
        fs::create_dir_all(root.join("memory-bank")).expect("create memory");

        let signals = local_context_signals(&root);
        for category in [
            "agents", "rules", "skills", "mcp", "workflow", "hook", "memory",
        ] {
            assert!(
                signals.categories.contains(&category.to_string()),
                "{category} should be detected from marker files"
            );
        }
        assert!(signals
            .evidence
            .iter()
            .any(|item| item.as_str() == Some("AGENTS.md")));
        assert!(signals
            .evidence
            .iter()
            .any(|item| item.as_str() == Some(".github/workflows")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn context_projects_report_git_scope() {
        let root = temp_test_dir("context-git-scope");
        let repo = root.join("repo-with-context");
        fs::create_dir_all(&repo).expect("create repo dir");
        let init = run("git", &["init"], Some(&repo));
        assert!(
            init.status.success(),
            "git init failed: {}",
            process_message(&init)
        );
        fs::write(repo.join("AGENTS.md"), "").expect("write agents marker");
        let nested_skill = repo.join("nested-skill");
        fs::create_dir_all(&nested_skill).expect("create nested skill");
        fs::write(nested_skill.join("SKILL.md"), "").expect("write skill marker");
        let no_git = root.join("loose-mcp");
        fs::create_dir_all(&no_git).expect("create loose mcp");
        fs::write(no_git.join(".mcp.json"), "{}").expect("write mcp marker");

        let local_repos = vec![inspect_local_repo(&repo, false)];
        let projects = find_context_projects(&[root.clone()], 5, &local_repos);
        let find_by_path = |target: &Path| {
            projects
                .iter()
                .find(|item| {
                    item.get("path")
                        .and_then(Value::as_str)
                        .map(|path| {
                            normalize_path_key(Path::new(path)) == normalize_path_key(target)
                        })
                        .unwrap_or(false)
                })
                .expect("context project")
        };

        assert_eq!(
            find_by_path(&repo).get("gitScope").and_then(Value::as_str),
            Some("self")
        );
        assert_eq!(
            find_by_path(&nested_skill)
                .get("gitScope")
                .and_then(Value::as_str),
            Some("inside")
        );
        assert_eq!(
            find_by_path(&no_git)
                .get("gitScope")
                .and_then(Value::as_str),
            Some("none")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scan_roots_merge_requested_and_defaults() {
        let roots = merge_scan_roots(
            vec![PathBuf::from("D:\\study\\code")],
            vec![
                PathBuf::from("D:\\study\\code"),
                PathBuf::from("D:\\study\\code\\0ai\\产品"),
            ],
        );
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0], PathBuf::from("D:\\study\\code"));
        assert_eq!(roots[1], PathBuf::from("D:\\study\\code\\0ai\\产品"));
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
            "localOnly": [],
            "localProjects": [
                {
                    "id": "context-1",
                    "name": "example-skill",
                    "path": "D:\\code\\example-skill",
                    "categories": ["rules"],
                    "contextKinds": ["rules"],
                    "evidence": ["AGENTS.md"],
                    "gitScope": "self",
                    "isGitRepo": true,
                    "remoteKeys": ["repoatlas/example-skill"]
                }
            ]
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
            Some("agents")
        );
        assert_eq!(
            rows[0]
                .get("categories")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
        assert!(rows[0]
            .get("categories")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("rules")));
        assert!(rows[0]
            .get("categories")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some("skills")));
        assert_eq!(
            rows[0]
                .get("localContextMatches")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(rows[1].get("category").and_then(Value::as_str), Some("mcp"));
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = env::temp_dir().join(format!("repo-atlas-{name}-{}-{nanos}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
