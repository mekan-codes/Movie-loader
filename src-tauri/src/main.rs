#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Utc;
use futures::channel::oneshot;
use futures::future::join_all;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use rusqlite::{params, Connection, OptionalExtension, Row};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io,
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder},
    Emitter, Manager, State, WebviewUrl,
};
use tokio::time::{sleep, timeout};
use url::Url;

const DEFAULT_USER_AGENT: &str = "CineFinder/0.1 local desktop aggregator";
const VIEWER_EVENT: &str = "cinefinder://viewer-state";

struct AppState {
    db: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceConfig {
    id: String,
    name: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    default_source_id: Option<String>,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    user_modified: bool,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    is_deleted: bool,
    #[serde(default)]
    deleted_at: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    #[serde(default = "default_source_type")]
    source_type: String,
    #[serde(default = "default_source_open_behavior")]
    source_open_behavior: String,
    #[serde(default = "default_result_open_behavior")]
    result_open_behavior: String,
    #[serde(default = "default_ambiguous_query_behavior")]
    ambiguous_query_behavior: String,
    #[serde(default = "default_parser_mode")]
    parser_mode: String,
    base_url: String,
    search_url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    result_selector: String,
    #[serde(default = "default_load_delay_ms")]
    load_delay_ms: u64,
    #[serde(default = "default_max_retries")]
    max_retries: u32,
    #[serde(default = "default_request_timeout_ms")]
    request_timeout_ms: u64,
    #[serde(default)]
    wait_for_selector: Option<String>,
    #[serde(default)]
    title_selector: Option<String>,
    #[serde(default)]
    poster_selector: Option<String>,
    #[serde(default)]
    poster_attribute: Option<String>,
    #[serde(default)]
    link_selector: Option<String>,
    #[serde(default)]
    link_attribute: Option<String>,
    #[serde(default)]
    year_selector: Option<String>,
    #[serde(default)]
    description_selector: Option<String>,
    #[serde(default)]
    video_selector: Option<String>,
    #[serde(default)]
    video_attribute: Option<String>,
    #[serde(default)]
    iframe_selector: Option<String>,
    #[serde(default)]
    iframe_attribute: Option<String>,
    #[serde(default)]
    subtitle_selector: Option<String>,
    #[serde(default)]
    subtitle_attribute: Option<String>,
    #[serde(default)]
    subtitle_language_attribute: Option<String>,
    #[serde(default)]
    audio_language_selector: Option<String>,
    #[serde(default)]
    download_selector: Option<String>,
    #[serde(default)]
    download_attribute: Option<String>,
    #[serde(default)]
    watch_button_selector: Option<String>,
    #[serde(default = "default_watch_link_text_patterns")]
    watch_link_text_patterns: Vec<String>,
    #[serde(default)]
    episode_selector: Option<String>,
    #[serde(default)]
    season_selector: Option<String>,
    #[serde(default)]
    player_selector: Option<String>,
    #[serde(default = "default_true")]
    auto_resolve_watch_page: bool,
    #[serde(default)]
    auto_open_first_watch_link: bool,
    #[serde(default = "default_true")]
    auto_open_best_match: bool,
    #[serde(default = "default_true")]
    auto_open_watch_button: bool,
    #[serde(default = "default_max_watch_resolve_steps")]
    max_watch_resolve_steps: u32,
    #[serde(default = "default_max_watch_resolve_steps")]
    max_resolve_steps: u32,
    #[serde(default = "default_load_delay_ms")]
    resolve_delay_ms: u64,
    #[serde(default = "default_exact_match_threshold")]
    exact_match_threshold: u32,
    #[serde(default)]
    requires_javascript: bool,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    id: String,
    source_id: String,
    source_name: String,
    title: String,
    url: String,
    open_mode: Option<String>,
    playable_url: Option<String>,
    poster_url: Option<String>,
    year: Option<String>,
    description: Option<String>,
    confidence: f64,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    source_reliability: Option<f64>,
    #[serde(default)]
    debug_info: Option<SourceDebugInfo>,
    raw_data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceSearchOutcome {
    source_id: String,
    source_name: String,
    status: String,
    message: Option<String>,
    elapsed_ms: u128,
    results: Vec<SearchResult>,
    debug_info: Option<SourceDebugInfo>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct SourceDebugInfo {
    generated_search_url: Option<String>,
    final_loaded_url: Option<String>,
    parser_mode_used: Option<String>,
    static_fetch_worked: Option<bool>,
    static_parse_worked: Option<bool>,
    web_view_parse_worked: Option<bool>,
    html_length: Option<usize>,
    result_container_count: Option<usize>,
    candidate_titles: Vec<String>,
    candidate_links: Vec<String>,
    best_score: Option<f64>,
    why_auto_open_failed: Option<String>,
    javascript_probably_required: Option<bool>,
    browser_preview_limited: Option<bool>,
    timeout_or_error: Option<String>,
    final_action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceTestResult {
    ok: bool,
    message: String,
    result_count: usize,
    elapsed_ms: u128,
    final_search_url: Option<String>,
    raw_status: Option<String>,
    selector_match_count: usize,
    preview_results: Vec<SourcePreviewResult>,
    fallback_used: bool,
    best_match: Option<SourcePreviewResult>,
    final_open_url: Option<String>,
    query_specificity: Option<String>,
    ambiguous: bool,
    detected_selectors: Vec<SelectorCandidate>,
    debug_info: Option<SourceDebugInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourcePreviewResult {
    title: String,
    url: String,
    year: Option<String>,
    score: Option<f64>,
    confidence_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectorCandidate {
    selector_type: String,
    selector: String,
    match_count: usize,
    sample: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Favorite {
    id: String,
    title: String,
    source_name: String,
    url: String,
    #[serde(default)]
    open_mode: Option<String>,
    #[serde(default)]
    playable_url: Option<String>,
    #[serde(default)]
    poster_url: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryItem {
    id: String,
    title: String,
    source_name: String,
    url: String,
    #[serde(default)]
    open_mode: Option<String>,
    #[serde(default)]
    playable_url: Option<String>,
    #[serde(default)]
    poster_url: Option<String>,
    #[serde(default)]
    last_opened_at: Option<String>,
    #[serde(default)]
    playback_position_seconds: f64,
    #[serde(default)]
    duration_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewerBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerOpenResult {
    final_url: String,
    resolved: bool,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerEventPayload {
    label: String,
    status: String,
    url: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewerResolveProbe {
    kind: String,
    url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebviewParsePayload {
    final_url: String,
    html_length: usize,
    results: Vec<WebviewCandidatePayload>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebviewCandidatePayload {
    title: String,
    url: String,
    year: Option<String>,
    poster_url: Option<String>,
    description: Option<String>,
    result_selector: Option<String>,
    result_index: usize,
}

#[derive(Debug, Clone)]
struct WebviewParsedResults {
    final_url: String,
    html_length: usize,
    results: Vec<SearchResult>,
}

fn default_enabled() -> bool {
    true
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_source_kind() -> String {
    "web".to_string()
}

fn default_source_type() -> String {
    "search".to_string()
}

fn default_source_open_behavior() -> String {
    "webview".to_string()
}

fn default_result_open_behavior() -> String {
    "result_page".to_string()
}

fn default_ambiguous_query_behavior() -> String {
    "show_choices".to_string()
}

fn default_parser_mode() -> String {
    "hybrid".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_watch_resolve_steps() -> u32 {
    2
}

fn default_exact_match_threshold() -> u32 {
    85
}

fn default_watch_link_text_patterns() -> Vec<String> {
    vec![
        "watch full movie".to_string(),
        "watch online".to_string(),
        "watch now".to_string(),
        "play".to_string(),
        "start watching".to_string(),
        "смотреть".to_string(),
        "смотреть онлайн".to_string(),
    ]
}

fn default_load_delay_ms() -> u64 {
    1500
}

fn default_max_retries() -> u32 {
    2
}

fn default_request_timeout_ms() -> u64 {
    15000
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_dir)?;
            let conn = Connection::open(app_dir.join("cinefinder.sqlite3"))?;
            init_db(&conn).map_err(io::Error::other)?;
            app.manage(AppState {
                db: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sources,
            save_source,
            delete_source,
            reset_default_source,
            restore_default_sources,
            reset_all_default_sources,
            test_source,
            search_sources,
            list_favorites,
            add_favorite,
            remove_favorite,
            list_history,
            record_history,
            remove_history_item,
            clear_history,
            restore_source,
            permanently_delete_source,
            empty_source_trash,
            open_external_url,
            open_viewer_webview,
            close_viewer_webview,
            resize_viewer_webview,
            reload_viewer_webview,
            go_back_viewer_webview,
            go_forward_viewer_webview
        ])
        .run(tauri::generate_context!())
        .expect("error while running CineFinder");
}

#[tauri::command]
fn list_sources(state: State<'_, AppState>) -> Result<Vec<SourceConfig>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    read_sources(&conn)
}

#[tauri::command]
fn save_source(
    state: State<'_, AppState>,
    source: SourceConfig,
) -> Result<SourceConfig, String> {
    let source = SourceConfig {
        user_modified: if source.is_default || source.default_source_id.is_some() {
            true
        } else {
            source.user_modified
        },
        ..source
    };
    validate_source(&source)?;
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    save_source_to_db(&conn, source)
}

#[tauri::command]
fn delete_source(state: State<'_, AppState>, source_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let target = read_source_by_id(&conn, &source_id)?;
    save_source_to_db(
        &conn,
        SourceConfig {
            hidden: false,
            is_deleted: true,
            deleted_at: Some(Utc::now().to_rfc3339()),
            user_modified: if target.is_default || target.default_source_id.is_some() {
                true
            } else {
                target.user_modified
            },
            ..target
        },
    )?;
    Ok(())
}

#[tauri::command]
fn restore_source(state: State<'_, AppState>, source_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let target = read_source_by_id(&conn, &source_id)?;
    save_source_to_db(
        &conn,
        SourceConfig {
            hidden: false,
            is_deleted: false,
            deleted_at: None,
            ..target
        },
    )?;
    Ok(())
}

#[tauri::command]
fn permanently_delete_source(state: State<'_, AppState>, source_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let target = read_source_by_id(&conn, &source_id)?;
    if let Some(default_source_id) = target.default_source_id.as_deref() {
        remember_deleted_default_source(&conn, default_source_id)?;
    }
    conn.execute("DELETE FROM sources WHERE id = ?1", params![source_id])
        .map_err(|error| format!("Could not permanently delete source: {error}"))?;
    Ok(())
}

#[tauri::command]
fn empty_source_trash(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let sources = read_sources(&conn)?;
    for source in sources.iter().filter(|source| source.is_deleted) {
        if let Some(default_source_id) = source.default_source_id.as_deref() {
            remember_deleted_default_source(&conn, default_source_id)?;
        }
    }
    for source in sources.into_iter().filter(|source| source.is_deleted) {
        conn.execute("DELETE FROM sources WHERE id = ?1", params![source.id])
            .map_err(|error| format!("Could not empty trash: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn reset_default_source(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<SourceConfig, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let current = read_source_by_id(&conn, &source_id)?;
    let default_source_id = current
        .default_source_id
        .as_deref()
        .ok_or_else(|| "This source is not a built-in default.".to_string())?;
    let default_source = default_source_by_id(default_source_id)
        .ok_or_else(|| "No built-in default config found for this source.".to_string())?;

    save_source_to_db(
        &conn,
        SourceConfig {
            id: current.id,
            created_at: current.created_at,
            updated_at: current.updated_at,
            hidden: false,
            is_deleted: false,
            deleted_at: None,
            ..default_source
        },
    )
}

#[tauri::command]
fn restore_default_sources(state: State<'_, AppState>) -> Result<Vec<SourceConfig>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    migrate_default_sources(&conn, true, false)?;
    read_sources(&conn)
}

#[tauri::command]
fn reset_all_default_sources(state: State<'_, AppState>) -> Result<Vec<SourceConfig>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    migrate_default_sources(&conn, true, true)?;
    read_sources(&conn)
}

#[tauri::command]
async fn test_source(source: SourceConfig, query: Option<String>) -> Result<SourceTestResult, String> {
    validate_source(&source)?;
    let started = Instant::now();
    let sample_query = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "gravity falls".to_string());
    Ok(test_direct_source(source, sample_query, started).await)
}

#[tauri::command]
async fn search_sources(
    window: tauri::Window,
    state: State<'_, AppState>,
    query: String,
    source_ids: Option<Vec<String>>,
) -> Result<Vec<SourceSearchOutcome>, String> {
    let trimmed_query = query.trim().to_string();
    if trimmed_query.is_empty() {
        return Ok(Vec::new());
    }

    let sources = {
        let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
        read_sources(&conn)?
    };

    let selected_ids: Option<HashSet<String>> = source_ids.and_then(|ids| {
        if ids.is_empty() {
            None
        } else {
            Some(ids.into_iter().collect())
        }
    });

    let searchable_sources = sources
        .into_iter()
        .filter(|source| source.enabled)
        .filter(|source| !source.hidden)
        .filter(|source| !source.is_deleted)
        .filter(|source| {
            selected_ids
                .as_ref()
                .map(|ids| ids.contains(&source.id))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    let tasks = searchable_sources
        .into_iter()
        .map(|source| search_single_source(window.clone(), source, trimmed_query.clone()));
    let mut outcomes = join_all(tasks).await;
    outcomes.sort_by(compare_outcomes);
    Ok(outcomes)
}

#[tauri::command]
fn list_favorites(state: State<'_, AppState>) -> Result<Vec<Favorite>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, source_name, url, open_mode, playable_url, poster_url, created_at
             FROM favorites
             ORDER BY created_at DESC",
        )
        .map_err(|error| format!("Could not read favorites: {error}"))?;
    let rows = stmt
        .query_map([], favorite_from_row)
        .map_err(|error| format!("Could not read favorites: {error}"))?;
    collect_rows(rows, "favorites")
}

#[tauri::command]
fn add_favorite(state: State<'_, AppState>, favorite: Favorite) -> Result<Favorite, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let now = Utc::now().to_rfc3339();
    let saved = Favorite {
        id: if favorite.id.trim().is_empty() {
            stable_id(&favorite.url)
        } else {
            favorite.id
        },
        title: favorite.title,
        source_name: favorite.source_name,
        url: favorite.url,
        open_mode: favorite.open_mode,
        playable_url: favorite.playable_url,
        poster_url: favorite.poster_url,
        created_at: Some(favorite.created_at.unwrap_or(now.clone())),
    };

    conn.execute(
        "INSERT INTO favorites (id, title, source_name, url, open_mode, playable_url, poster_url, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(url) DO UPDATE SET
           title = excluded.title,
           source_name = excluded.source_name,
           open_mode = excluded.open_mode,
           playable_url = excluded.playable_url,
           poster_url = excluded.poster_url",
        params![
            saved.id,
            saved.title,
            saved.source_name,
            saved.url,
            saved.open_mode,
            saved.playable_url,
            saved.poster_url,
            saved.created_at
        ],
    )
    .map_err(|error| format!("Could not save favorite: {error}"))?;

    Ok(saved)
}

#[tauri::command]
fn remove_favorite(state: State<'_, AppState>, favorite_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    conn.execute("DELETE FROM favorites WHERE id = ?1", params![favorite_id])
        .map_err(|error| format!("Could not remove favorite: {error}"))?;
    Ok(())
}

#[tauri::command]
fn list_history(state: State<'_, AppState>) -> Result<Vec<HistoryItem>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, source_name, url, open_mode, playable_url, poster_url, last_opened_at,
                    playback_position_seconds, duration_seconds
             FROM history
             ORDER BY last_opened_at DESC",
        )
        .map_err(|error| format!("Could not read history: {error}"))?;
    let rows = stmt
        .query_map([], history_from_row)
        .map_err(|error| format!("Could not read history: {error}"))?;
    collect_rows(rows, "history")
}

#[tauri::command]
fn record_history(state: State<'_, AppState>, item: HistoryItem) -> Result<HistoryItem, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let now = Utc::now().to_rfc3339();
    let saved = HistoryItem {
        id: if item.id.trim().is_empty() {
            stable_id(&item.url)
        } else {
            item.id
        },
        title: item.title,
        source_name: item.source_name,
        url: item.url,
        open_mode: item.open_mode,
        playable_url: item.playable_url,
        poster_url: item.poster_url,
        last_opened_at: Some(item.last_opened_at.unwrap_or(now.clone())),
        playback_position_seconds: item.playback_position_seconds,
        duration_seconds: item.duration_seconds,
    };

    conn.execute(
        "INSERT INTO history
           (id, title, source_name, url, open_mode, playable_url, poster_url, last_opened_at,
            playback_position_seconds, duration_seconds)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(url) DO UPDATE SET
           title = excluded.title,
           source_name = excluded.source_name,
           open_mode = excluded.open_mode,
           playable_url = excluded.playable_url,
           poster_url = excluded.poster_url,
           last_opened_at = excluded.last_opened_at,
           playback_position_seconds = excluded.playback_position_seconds,
           duration_seconds = excluded.duration_seconds",
        params![
            saved.id,
            saved.title,
            saved.source_name,
            saved.url,
            saved.open_mode,
            saved.playable_url,
            saved.poster_url,
            saved.last_opened_at,
            saved.playback_position_seconds,
            saved.duration_seconds
        ],
    )
    .map_err(|error| format!("Could not save history: {error}"))?;

    Ok(saved)
}

#[tauri::command]
fn remove_history_item(state: State<'_, AppState>, history_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    conn.execute("DELETE FROM history WHERE id = ?1", params![history_id])
        .map_err(|error| format!("Could not remove history item: {error}"))?;
    Ok(())
}

#[tauri::command]
fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    conn.execute("DELETE FROM history", [])
        .map_err(|error| format!("Could not clear history: {error}"))?;
    Ok(())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    let parsed = Url::parse(url.trim()).map_err(|_| "URL must be absolute.".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("Only http and https URLs can be opened.".to_string()),
    }

    open_url_with_system(parsed.as_str())
}

#[tauri::command]
async fn open_viewer_webview(
    window: tauri::Window,
    label: String,
    url: String,
    bounds: ViewerBounds,
    source: SourceConfig,
) -> Result<ViewerOpenResult, String> {
    let parsed = parse_external_http_url(&url)?;
    let app = window.app_handle().clone();
    close_viewer_by_label(&app, &label);

    emit_viewer_state(
        &app,
        &label,
        "Opening result page",
        Some(parsed.as_str().to_string()),
        None,
    );

    let resolving = source.auto_resolve_watch_page;
    let initial_bounds = if resolving {
        ViewerBounds {
            x: -32000.0,
            y: -32000.0,
            width: 1.0,
            height: 1.0,
        }
    } else {
        bounds.clone()
    };
    let window_label = window.label().to_string();
    let app_for_page = app.clone();
    let page_label = label.clone();
    let app_for_popup = app.clone();
    let popup_label = label.clone();
    let app_for_nav = app.clone();
    let nav_label = label.clone();

    let builder = WebviewBuilder::new(label.clone(), WebviewUrl::External(parsed.clone()))
        .initialization_script_for_all_frames(viewer_initialization_script())
        .on_page_load(move |_webview, payload| {
            let status = match payload.event() {
                PageLoadEvent::Started => "Opening result page",
                PageLoadEvent::Finished => "Ready",
            };
            let _ = app_for_page.emit_to(
                &window_label,
                VIEWER_EVENT,
                ViewerEventPayload {
                    label: page_label.clone(),
                    status: status.to_string(),
                    url: Some(payload.url().to_string()),
                    message: None,
                },
            );
        })
        .on_new_window(move |popup_url, _features| {
            if let Some(webview) = app_for_popup.get_webview(&popup_label) {
                let _ = webview.navigate(popup_url.clone());
            }
            emit_viewer_state(
                &app_for_popup,
                &popup_label,
                "Opening watch page",
                Some(popup_url.to_string()),
                None,
            );
            NewWindowResponse::Deny
        })
        .on_navigation(move |navigation_url| {
            emit_viewer_state(
                &app_for_nav,
                &nav_label,
                "Opening result page",
                Some(navigation_url.to_string()),
                None,
            );
            true
        });

    let webview = window
        .add_child(
            builder,
            tauri::LogicalPosition::new(initial_bounds.x, initial_bounds.y),
            tauri::LogicalSize::new(initial_bounds.width, initial_bounds.height),
        )
        .map_err(|error| format!("Could not open in-app viewer: {error}"))?;

    if resolving {
        let _ = webview.hide();
    }

    let result = resolve_viewer_webview(&app, &label, &webview, &source, parsed).await;

    webview
        .set_position(tauri::LogicalPosition::new(bounds.x, bounds.y))
        .map_err(|error| format!("Could not position viewer: {error}"))?;
    webview
        .set_size(tauri::LogicalSize::new(bounds.width, bounds.height))
        .map_err(|error| format!("Could not size viewer: {error}"))?;
    let _ = webview.show();

    match result {
        Ok(result) => {
            emit_viewer_state(
                &app,
                &label,
                &result.status,
                Some(result.final_url.clone()),
                None,
            );
            Ok(result)
        }
        Err(error) => {
            let final_url = webview
                .url()
                .map(|url| url.to_string())
                .unwrap_or_else(|_| url.clone());
            emit_viewer_state(
                &app,
                &label,
                "Could not auto-resolve, showing result page",
                Some(final_url.clone()),
                Some(error),
            );
            Ok(ViewerOpenResult {
                final_url,
                resolved: false,
                status: "Could not auto-resolve, showing result page".to_string(),
            })
        }
    }
}

#[tauri::command]
fn close_viewer_webview(window: tauri::Window, label: String) -> Result<(), String> {
    close_viewer_by_label(window.app_handle(), &label);
    Ok(())
}

#[tauri::command]
fn resize_viewer_webview(
    window: tauri::Window,
    label: String,
    bounds: ViewerBounds,
) -> Result<(), String> {
    let Some(webview) = window.app_handle().get_webview(&label) else {
        return Ok(());
    };
    webview
        .set_position(tauri::LogicalPosition::new(bounds.x, bounds.y))
        .map_err(|error| format!("Could not position viewer: {error}"))?;
    webview
        .set_size(tauri::LogicalSize::new(bounds.width, bounds.height))
        .map_err(|error| format!("Could not size viewer: {error}"))?;
    Ok(())
}

#[tauri::command]
fn reload_viewer_webview(window: tauri::Window, label: String) -> Result<(), String> {
    if let Some(webview) = window.app_handle().get_webview(&label) {
        webview
            .reload()
            .map_err(|error| format!("Could not refresh viewer: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn go_back_viewer_webview(window: tauri::Window, label: String) -> Result<(), String> {
    if let Some(webview) = window.app_handle().get_webview(&label) {
        webview
            .eval("window.history.back();")
            .map_err(|error| format!("Could not navigate back: {error}"))?;
    }
    Ok(())
}

#[tauri::command]
fn go_forward_viewer_webview(window: tauri::Window, label: String) -> Result<(), String> {
    if let Some(webview) = window.app_handle().get_webview(&label) {
        webview
            .eval("window.history.forward();")
            .map_err(|error| format!("Could not navigate forward: {error}"))?;
    }
    Ok(())
}

async fn resolve_viewer_webview(
    app: &tauri::AppHandle,
    label: &str,
    webview: &tauri::Webview,
    source: &SourceConfig,
    original_url: Url,
) -> Result<ViewerOpenResult, String> {
    if !source.auto_resolve_watch_page {
        wait_for_viewer_ready(webview, source_timeout_ms(source)).await;
        let final_url = current_webview_url(webview, original_url.as_str());
        return Ok(ViewerOpenResult {
            final_url,
            resolved: false,
            status: "Ready".to_string(),
        });
    }

    let original_url_string = original_url.to_string();
    let mut opened_watch_page = false;
    let mut found_player = false;
    let max_steps = source.max_resolve_steps.min(5);

    wait_for_viewer_ready(webview, source_timeout_ms(source)).await;

    for step in 0..=max_steps {
        emit_viewer_state(
            app,
            label,
            "Looking for player",
            Some(current_webview_url(webview, &original_url_string)),
            None,
        );
        let delay_ms = source.resolve_delay_ms.min(10_000);
        if delay_ms > 0 {
            sleep(Duration::from_millis(delay_ms)).await;
        }

        let probe = probe_viewer_watch_state(webview, source).await?;
        if probe.kind == "player" {
            found_player = true;
            break;
        }
        if step >= max_steps {
            break;
        }

        match probe.kind.as_str() {
            "link" => {
                let Some(next_url) = probe.url.as_deref() else {
                    break;
                };
                let next_url = parse_external_http_url(next_url)?;
                emit_viewer_state(
                    app,
                    label,
                    "Opening watch page",
                    Some(next_url.to_string()),
                    None,
                );
                webview
                    .navigate(next_url)
                    .map_err(|error| format!("Could not open watch page: {error}"))?;
                opened_watch_page = true;
                wait_for_viewer_ready(webview, source_timeout_ms(source)).await;
            }
            "click" => {
                emit_viewer_state(
                    app,
                    label,
                    "Opening watch page",
                    Some(current_webview_url(webview, &original_url_string)),
                    None,
                );
                webview
                    .eval(viewer_click_candidate_script())
                    .map_err(|error| format!("Could not click watch button: {error}"))?;
                opened_watch_page = true;
                wait_for_viewer_ready(webview, source_timeout_ms(source)).await;
            }
            _ => break,
        }
    }

    if !found_player && opened_watch_page {
        let _ = webview.navigate(original_url.clone());
        wait_for_viewer_ready(webview, source_timeout_ms(source)).await;
    }

    let final_url = current_webview_url(webview, original_url.as_str());
    Ok(ViewerOpenResult {
        final_url,
        resolved: found_player,
        status: if found_player {
            "Ready".to_string()
        } else {
            "Could not auto-resolve, showing result page".to_string()
        },
    })
}

async fn probe_viewer_watch_state(
    webview: &tauri::Webview,
    source: &SourceConfig,
) -> Result<ViewerResolveProbe, String> {
    let script = viewer_probe_script(source)?;
    let value = eval_webview_json(webview, script, 2_500).await?;
    serde_json::from_str::<ViewerResolveProbe>(&value)
        .map_err(|error| format!("Could not read viewer resolver result: {error}"))
}

async fn wait_for_viewer_ready(webview: &tauri::Webview, timeout_ms: u64) {
    let timeout_at = Instant::now() + Duration::from_millis(timeout_ms.min(60_000));
    while Instant::now() < timeout_at {
        if let Ok(value) = eval_webview_json(
            webview,
            "(() => document.readyState)()".to_string(),
            1_000,
        )
        .await
        {
            if let Ok(state) = serde_json::from_str::<String>(&value) {
                if state == "interactive" || state == "complete" {
                    return;
                }
            }
        }
        sleep(Duration::from_millis(150)).await;
    }
}

async fn eval_webview_json(
    webview: &tauri::Webview,
    script: String,
    timeout_ms: u64,
) -> Result<String, String> {
    let (sender, receiver) = oneshot::channel::<String>();
    let sender = Mutex::new(Some(sender));
    webview
        .eval_with_callback(script, move |value| {
            if let Ok(mut sender) = sender.lock() {
                if let Some(sender) = sender.take() {
                    let _ = sender.send(value);
                }
            }
        })
        .map_err(|error| format!("Could not inspect viewer page: {error}"))?;

    timeout(Duration::from_millis(timeout_ms), receiver)
        .await
        .map_err(|_| "Timed out inspecting viewer page.".to_string())?
        .map_err(|_| "Viewer inspection callback was cancelled.".to_string())
}

async fn parse_search_with_webview(
    window: &tauri::Window,
    source: &SourceConfig,
    query: &str,
    search_url: &str,
) -> Result<WebviewParsedResults, String> {
    let parsed = parse_external_http_url(search_url)?;
    let label = format!("cinefinder-search-{}-{}", source.id, stable_id(search_url));
    let app = window.app_handle().clone();
    close_viewer_by_label(&app, &label);

    let builder = WebviewBuilder::new(label.clone(), WebviewUrl::External(parsed))
        .initialization_script_for_all_frames(viewer_initialization_script())
        .on_new_window(|_, _| NewWindowResponse::Deny);

    let webview = window
        .add_child(
            builder,
            tauri::LogicalPosition::new(-32000.0, -32000.0),
            tauri::LogicalSize::new(1.0, 1.0),
        )
        .map_err(|error| format!("Could not open WebView parser: {error}"))?;
    let _ = webview.hide();

    wait_for_viewer_ready(&webview, source_timeout_ms(source)).await;
    if source.load_delay_ms > 0 {
        sleep(Duration::from_millis(source.load_delay_ms.min(10_000))).await;
    }

    let script = webview_search_parse_script(source)?;
    let value = eval_webview_json(&webview, script, source_timeout_ms(source)).await;
    let _ = webview.close();

    let payload = serde_json::from_str::<WebviewParsePayload>(&value?)
        .map_err(|error| format!("Could not parse rendered search results: {error}"))?;

    let mut seen = HashSet::new();
    let mut results = payload
        .results
        .into_iter()
        .filter_map(|candidate| {
            if candidate.title.trim().is_empty() || candidate.url.trim().is_empty() {
                return None;
            }
            let dedupe_key = format!(
                "{}:{}:{}",
                source.id,
                normalize_match_key(&candidate.title),
                candidate.year.clone().unwrap_or_default()
            );
            if !seen.insert(dedupe_key) {
                return None;
            }
            let scored = score_result_candidate(query, &candidate.title, candidate.year.as_deref());
            if scored.score < 12.0 {
                return None;
            }
            let quality = quality_for_score(scored.score, "parsed");
            let mut raw_data = HashMap::new();
            raw_data.insert("provider".to_string(), "rendered-webview".to_string());
            raw_data.insert("resultIndex".to_string(), candidate.result_index.to_string());
            if let Some(selector) = candidate.result_selector {
                raw_data.insert("resultSelector".to_string(), selector);
            }
            raw_data.insert("confidenceReason".to_string(), scored.reason.clone());
            raw_data.insert("quality".to_string(), quality.clone());
            Some(SearchResult {
                id: stable_id(&format!("{}:{}", source.id, candidate.url)),
                source_id: source.id.clone(),
                source_name: source.name.clone(),
                title: candidate.title,
                url: candidate.url,
                open_mode: Some("webview".to_string()),
                playable_url: None,
                poster_url: candidate.poster_url,
                year: candidate.year,
                description: candidate.description,
                confidence: scored.score,
                quality: Some(quality),
                reason: Some(scored.reason),
                source_reliability: Some(source_reliability(source)),
                debug_info: None,
                raw_data,
            })
        })
        .collect::<Vec<_>>();

    results.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(24);

    Ok(WebviewParsedResults {
        final_url: payload.final_url,
        html_length: payload.html_length,
        results,
    })
}

fn webview_search_parse_script(source: &SourceConfig) -> Result<String, String> {
    let result_selectors = if source.result_selector.trim().is_empty() {
        common_result_selectors()
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    } else {
        vec![source.result_selector.trim().to_string()]
    };
    let title_selectors = source
        .title_selector
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value.trim().to_string()])
        .unwrap_or_else(|| common_title_selectors().iter().map(|value| value.to_string()).collect());
    let link_selectors = source
        .link_selector
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value.trim().to_string()])
        .unwrap_or_else(|| common_link_selectors().iter().map(|value| value.to_string()).collect());
    let poster_selectors = source
        .poster_selector
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value.trim().to_string()])
        .unwrap_or_else(|| common_poster_selectors().iter().map(|value| value.to_string()).collect());
    let year_selectors = source
        .year_selector
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![value.trim().to_string()])
        .unwrap_or_else(|| common_year_selectors().iter().map(|value| value.to_string()).collect());
    let description_selector = source.description_selector.clone().unwrap_or_default();
    let link_attribute = source
        .link_attribute
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "href".to_string());
    let poster_attribute = source
        .poster_attribute
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "src".to_string());

    let result_selectors_json = serde_json::to_string(&result_selectors)
        .map_err(|error| format!("Could not serialize result selectors: {error}"))?;
    let title_selectors_json = serde_json::to_string(&title_selectors)
        .map_err(|error| format!("Could not serialize title selectors: {error}"))?;
    let link_selectors_json = serde_json::to_string(&link_selectors)
        .map_err(|error| format!("Could not serialize link selectors: {error}"))?;
    let poster_selectors_json = serde_json::to_string(&poster_selectors)
        .map_err(|error| format!("Could not serialize poster selectors: {error}"))?;
    let year_selectors_json = serde_json::to_string(&year_selectors)
        .map_err(|error| format!("Could not serialize year selectors: {error}"))?;
    let description_selector_json = serde_json::to_string(&description_selector)
        .map_err(|error| format!("Could not serialize description selector: {error}"))?;
    let link_attribute_json = serde_json::to_string(&link_attribute)
        .map_err(|error| format!("Could not serialize link attribute: {error}"))?;
    let poster_attribute_json = serde_json::to_string(&poster_attribute)
        .map_err(|error| format!("Could not serialize poster attribute: {error}"))?;

    Ok(format!(
        r#"
(() => {{
  const resultSelectors = {result_selectors_json};
  const titleSelectors = {title_selectors_json};
  const linkSelectors = {link_selectors_json};
  const posterSelectors = {poster_selectors_json};
  const yearSelectors = {year_selectors_json};
  const descriptionSelector = {description_selector_json};
  const linkAttribute = {link_attribute_json};
  const posterAttribute = {poster_attribute_json};
  const clean = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const safeAll = (root, selector) => {{
    if (!selector || !String(selector).trim()) return [];
    try {{ return Array.from((root || document).querySelectorAll(selector)); }} catch (_) {{ return []; }}
  }};
  const firstMatching = (root, selectors) => {{
    for (const selector of selectors) {{
      try {{
        if (root.matches && root.matches(selector)) return root;
        const found = root.querySelector(selector);
        if (found) return found;
      }} catch (_) {{}}
    }}
    return null;
  }};
  const readAttr = (element, attr) => element ? clean(element.getAttribute(attr)) : "";
  const readAnyImageAttr = (element) => {{
    if (!element) return "";
    return readAttr(element, posterAttribute) || readAttr(element, "src") || readAttr(element, "data-src") ||
      readAttr(element, "data-original") || readAttr(element, "data-lazy-src");
  }};
  const textOf = (element) => clean(
    readAttr(element, "title") ||
    readAttr(element, "aria-label") ||
    readAttr(element, "alt") ||
    readAttr(element?.querySelector?.("img"), "alt") ||
    element?.textContent ||
    ""
  );
  const absoluteUrl = (value) => {{
    const raw = clean(value);
    if (!raw || raw.startsWith("javascript:") || raw.startsWith("mailto:") || raw.startsWith('#')) return null;
    try {{ return new URL(raw, window.location.href).toString(); }} catch (_) {{ return null; }}
  }};
  const extractYear = (value) => {{
    const match = String(value || "").match(/\b(19|20)\d{{2}}\b/);
    return match ? match[0] : null;
  }};
  const results = [];
  const seen = new Set();
  for (const resultSelector of resultSelectors) {{
    const cards = safeAll(document, resultSelector).slice(0, 100);
    cards.forEach((card, index) => {{
      const linkElement = firstMatching(card, linkSelectors) || (card.matches?.("a[href]") ? card : null);
      const url = absoluteUrl(readAttr(linkElement, linkAttribute) || readAttr(linkElement, "href"));
      if (!url) return;
      const titleElement = firstMatching(card, titleSelectors);
      const title = textOf(titleElement) || textOf(linkElement) || textOf(firstMatching(card, ["img"])) || textOf(card) || url;
      const yearElement = firstMatching(card, yearSelectors);
      const year = textOf(yearElement) || extractYear(`${{title}} ${{card.textContent || ""}}`);
      const posterElement = firstMatching(card, posterSelectors);
      const posterUrl = absoluteUrl(readAnyImageAttr(posterElement));
      const description = descriptionSelector ? textOf(firstMatching(card, [descriptionSelector])) : "";
      const key = `${{title.toLowerCase()}}|${{year || ""}}|${{url}}`;
      if (seen.has(key)) return;
      seen.add(key);
      results.push({{
        title,
        url,
        year,
        posterUrl,
        description,
        resultSelector,
        resultIndex: index
      }});
    }});
  }}
  return {{
    finalUrl: window.location.href,
    htmlLength: document.documentElement.outerHTML.length,
    results: results.slice(0, 80)
  }};
}})()
"#
    ))
}

fn viewer_probe_script(source: &SourceConfig) -> Result<String, String> {
    let watch_selector = source.watch_button_selector.clone().unwrap_or_default();
    let player_selector = source
        .player_selector
        .clone()
        .unwrap_or_else(|| "video, iframe".to_string());
    let patterns = source
        .watch_link_text_patterns
        .iter()
        .map(|pattern| normalize_comparable_title(pattern))
        .filter(|pattern| !pattern.is_empty())
        .collect::<Vec<_>>();
    let watch_selector_json = serde_json::to_string(&watch_selector)
        .map_err(|error| format!("Could not serialize selector: {error}"))?;
    let player_selector_json = serde_json::to_string(&player_selector)
        .map_err(|error| format!("Could not serialize player selector: {error}"))?;
    let patterns_json = serde_json::to_string(&patterns)
        .map_err(|error| format!("Could not serialize watch text patterns: {error}"))?;

    Ok(format!(
        r#"
(() => {{
  const playerSelector = {player_selector_json};
  const watchButtonSelector = {watch_selector_json};
  const patterns = {patterns_json};
  const safeQueryAll = (selector) => {{
    if (!selector || !String(selector).trim()) return [];
    try {{ return Array.from(document.querySelectorAll(selector)); }} catch (_) {{ return []; }}
  }};
  const normalize = (value) => String(value || "")
    .toLowerCase()
    .replace(/[^\p{{L}}\p{{N}}]+/gu, " ")
    .replace(/\b(full|movie|watch|online|hd|free|season|episode|ep|show|film|смотреть|онлайн)\b/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
  const visible = (element) => {{
    if (!element) return false;
    const style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden" || style.pointerEvents === "none") return false;
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }};
  const elementUrl = (element) => {{
    const raw = element?.getAttribute("href") || element?.getAttribute("data-href") || element?.getAttribute("data-url");
    if (!raw || raw.startsWith("javascript:") || raw.startsWith("mailto:")) return null;
    try {{ return new URL(raw, window.location.href).toString(); }} catch (_) {{ return null; }}
  }};
  if (safeQueryAll(playerSelector).some(visible)) {{
    return {{ kind: "player", url: window.location.href }};
  }}

  const selectorMatches = safeQueryAll(watchButtonSelector).filter(visible);
  const textMatches = Array.from(document.querySelectorAll("a[href], button, [role='button'], [data-href], [data-url]"))
    .filter(visible)
    .filter((element) => {{
      const text = normalize([
        element.textContent || "",
        element.getAttribute("title") || "",
        element.getAttribute("aria-label") || ""
      ].join(" "));
      return text && patterns.some((pattern) => pattern && text.includes(pattern));
    }});
  const candidates = selectorMatches.length ? selectorMatches : textMatches;
  const match = candidates[0];
  if (!match) return {{ kind: "none", url: window.location.href }};
  const url = elementUrl(match);
  if (url) return {{ kind: "link", url }};
  document.querySelectorAll("[data-cinefinder-watch-target]").forEach((element) => element.removeAttribute("data-cinefinder-watch-target"));
  match.setAttribute("data-cinefinder-watch-target", "true");
  return {{ kind: "click", url: window.location.href }};
}})()
"#
    ))
}

fn viewer_click_candidate_script() -> String {
    r#"
(() => {
  const target = document.querySelector("[data-cinefinder-watch-target]");
  if (target) {
    target.removeAttribute("data-cinefinder-watch-target");
    target.click();
  }
})()
"#
    .to_string()
}

fn viewer_initialization_script() -> String {
    r#"
(() => {
  const openInPlace = (url) => {
    if (!url) return null;
    try {
      window.location.href = new URL(String(url), window.location.href).toString();
    } catch (_) {
      window.location.href = String(url);
    }
    return null;
  };
  window.open = openInPlace;
  const forceSameViewer = (root = document) => {
    try {
      root.querySelectorAll?.("a[target], form[target]").forEach((element) => {
        element.setAttribute("target", "_self");
      });
    } catch (_) {}
  };
  forceSameViewer();
  new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      mutation.addedNodes.forEach((node) => {
        if (node.nodeType === 1) forceSameViewer(node);
      });
    }
  }).observe(document.documentElement, { childList: true, subtree: true });
  document.addEventListener("click", (event) => {
    const target = event.target && event.target.closest ? event.target.closest("a[target], area[target]") : null;
    if (target && target.href) {
      event.preventDefault();
      openInPlace(target.href);
    }
  }, true);
})()
"#
    .to_string()
}

fn emit_viewer_state(
    app: &tauri::AppHandle,
    label: &str,
    status: &str,
    url: Option<String>,
    message: Option<String>,
) {
    let _ = app.emit(
        VIEWER_EVENT,
        ViewerEventPayload {
            label: label.to_string(),
            status: status.to_string(),
            url,
            message,
        },
    );
}

fn close_viewer_by_label(app: &tauri::AppHandle, label: &str) {
    if let Some(webview) = app.get_webview(label) {
        let _ = webview.close();
    }
}

fn current_webview_url(webview: &tauri::Webview, fallback: &str) -> String {
    webview
        .url()
        .map(|url| url.to_string())
        .unwrap_or_else(|_| fallback.to_string())
}

fn parse_external_http_url(value: &str) -> Result<Url, String> {
    let parsed = Url::parse(value.trim()).map_err(|_| "URL must be absolute.".to_string())?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err("Only http and https URLs can be opened.".to_string()),
    }
}

fn init_db(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sources (
          id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          enabled INTEGER NOT NULL,
          config_json TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS deleted_default_sources (
          default_source_id TEXT PRIMARY KEY,
          deleted_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS favorites (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          source_name TEXT NOT NULL,
          url TEXT NOT NULL UNIQUE,
          open_mode TEXT,
          playable_url TEXT,
          poster_url TEXT,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS history (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          source_name TEXT NOT NULL,
          url TEXT NOT NULL UNIQUE,
          open_mode TEXT,
          playable_url TEXT,
          poster_url TEXT,
          last_opened_at TEXT NOT NULL,
          playback_position_seconds REAL NOT NULL DEFAULT 0,
          duration_seconds REAL NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS downloads (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          source_name TEXT NOT NULL,
          url TEXT NOT NULL,
          file_path TEXT,
          status TEXT NOT NULL,
          progress REAL NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL
        );
        ",
    )
    .map_err(|error| format!("Could not initialize database: {error}"))?;

    let _ = conn.execute("ALTER TABLE favorites ADD COLUMN playable_url TEXT", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN playable_url TEXT", []);
    let _ = conn.execute("ALTER TABLE favorites ADD COLUMN open_mode TEXT", []);
    let _ = conn.execute("ALTER TABLE history ADD COLUMN open_mode TEXT", []);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))
        .map_err(|error| format!("Could not inspect sources: {error}"))?;

    if count == 0 {
        for source in sample_sources() {
            save_source_to_db(conn, source)?;
        }
    }
    migrate_default_sources(conn, false, false)?;

    Ok(())
}

fn read_sources(conn: &Connection) -> Result<Vec<SourceConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT config_json, created_at, updated_at
             FROM sources
             ORDER BY name COLLATE NOCASE ASC",
        )
        .map_err(|error| format!("Could not read sources: {error}"))?;

    let rows = stmt
        .query_map([], |row| {
            let json: String = row.get(0)?;
            let created_at: String = row.get(1)?;
            let updated_at: String = row.get(2)?;
            let mut source: SourceConfig = serde_json::from_str(&json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            source.created_at = Some(created_at);
            source.updated_at = Some(updated_at);
            Ok(source)
        })
        .map_err(|error| format!("Could not read sources: {error}"))?;

    collect_rows(rows, "sources")
}

fn read_source_by_id(conn: &Connection, source_id: &str) -> Result<SourceConfig, String> {
    let row = conn
        .query_row(
            "SELECT config_json, created_at, updated_at FROM sources WHERE id = ?1",
            params![source_id],
            |row| {
                let json: String = row.get(0)?;
                let created_at: String = row.get(1)?;
                let updated_at: String = row.get(2)?;
                let mut source: SourceConfig = serde_json::from_str(&json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                source.created_at = Some(created_at);
                source.updated_at = Some(updated_at);
                Ok(source)
            },
        )
        .optional()
        .map_err(|error| format!("Could not read source: {error}"))?;
    row.ok_or_else(|| "Source not found.".to_string())
}

fn save_source_to_db(
    conn: &Connection,
    source: SourceConfig,
) -> Result<SourceConfig, String> {
    let now = Utc::now().to_rfc3339();
    let created_at = conn
        .query_row(
            "SELECT created_at FROM sources WHERE id = ?1",
            params![source.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("Could not inspect source: {error}"))?
        .or(source.created_at.clone())
        .unwrap_or_else(|| now.clone());

    let normalized = SourceConfig {
        id: source.id.trim().to_string(),
        name: source.name.trim().to_string(),
        enabled: source.enabled,
        default_source_id: clean_string(source.default_source_id),
        is_default: source.is_default,
        user_modified: source.user_modified,
        hidden: source.hidden,
        is_deleted: source.is_deleted,
        deleted_at: if source.is_deleted {
            clean_string(source.deleted_at)
        } else {
            None
        },
        note: clean_string(source.note),
        source_kind: normalized_source_kind(&source.source_kind),
        source_type: normalized_source_type(&source.source_type),
        source_open_behavior: normalized_source_open_behavior(&source.source_open_behavior),
        result_open_behavior: normalized_result_open_behavior(&source.result_open_behavior),
        ambiguous_query_behavior: normalized_ambiguous_query_behavior(&source.ambiguous_query_behavior),
        parser_mode: normalized_parser_mode(&source.parser_mode, &source.source_type, source.requires_javascript),
        base_url: source.base_url.trim().to_string(),
        search_url: source.search_url.trim().to_string(),
        method: source.method.trim().to_uppercase(),
        result_selector: source.result_selector.trim().to_string(),
        load_delay_ms: source.load_delay_ms.min(10_000),
        max_retries: source.max_retries.min(5),
        request_timeout_ms: source.request_timeout_ms.clamp(3_000, 60_000),
        wait_for_selector: clean_string(source.wait_for_selector),
        title_selector: clean_string(source.title_selector),
        poster_selector: clean_string(source.poster_selector),
        poster_attribute: clean_string(source.poster_attribute),
        link_selector: clean_string(source.link_selector),
        link_attribute: clean_string(source.link_attribute),
        year_selector: clean_string(source.year_selector),
        description_selector: clean_string(source.description_selector),
        video_selector: clean_string(source.video_selector),
        video_attribute: clean_string(source.video_attribute),
        iframe_selector: clean_string(source.iframe_selector),
        iframe_attribute: clean_string(source.iframe_attribute),
        subtitle_selector: clean_string(source.subtitle_selector),
        subtitle_attribute: clean_string(source.subtitle_attribute),
        subtitle_language_attribute: clean_string(source.subtitle_language_attribute),
        audio_language_selector: clean_string(source.audio_language_selector),
        download_selector: clean_string(source.download_selector),
        download_attribute: clean_string(source.download_attribute),
        watch_button_selector: clean_string(source.watch_button_selector),
        watch_link_text_patterns: normalize_watch_patterns(source.watch_link_text_patterns),
        episode_selector: clean_string(source.episode_selector),
        season_selector: clean_string(source.season_selector),
        player_selector: clean_string(source.player_selector).or_else(|| Some("video, iframe".to_string())),
        auto_resolve_watch_page: source.auto_resolve_watch_page,
        auto_open_first_watch_link: source.auto_open_first_watch_link,
        auto_open_best_match: source.auto_open_best_match,
        auto_open_watch_button: source.auto_open_watch_button,
        max_watch_resolve_steps: source.max_resolve_steps.min(5),
        max_resolve_steps: source.max_resolve_steps.min(5),
        resolve_delay_ms: source.resolve_delay_ms.min(10_000),
        exact_match_threshold: source.exact_match_threshold.clamp(50, 100),
        requires_javascript: source.requires_javascript,
        headers: source.headers,
        created_at: Some(created_at.clone()),
        updated_at: Some(now.clone()),
    };

    let config_json = serde_json::to_string(&normalized)
        .map_err(|error| format!("Could not serialize source: {error}"))?;
    conn.execute(
        "INSERT INTO sources (id, name, enabled, config_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           enabled = excluded.enabled,
           config_json = excluded.config_json,
           updated_at = excluded.updated_at",
        params![
            normalized.id,
            normalized.name,
            bool_to_int(normalized.enabled),
            config_json,
            created_at,
            now
        ],
    )
    .map_err(|error| format!("Could not save source: {error}"))?;

    Ok(normalized)
}

fn migrate_default_sources(
    conn: &Connection,
    restore_hidden: bool,
    reset_all: bool,
) -> Result<(), String> {
    if restore_hidden || reset_all {
        conn.execute("DELETE FROM deleted_default_sources", [])
            .map_err(|error| format!("Could not restore deleted defaults: {error}"))?;
    }
    let sources = read_sources(conn)?;
    let deleted_default_ids = read_deleted_default_source_ids(conn)?;
    let mut seen_default_ids = HashSet::new();

    for source in sources {
        let source = migrate_legacy_deleted_source(infer_default_metadata(source));
        if is_removed_or_wrong_source(&source) {
            conn.execute("DELETE FROM sources WHERE id = ?1", params![source.id])
                .map_err(|error| format!("Could not remove old source preset: {error}"))?;
            continue;
        }

        if let Some(default_source_id) = source.default_source_id.clone() {
            if let Some(default_source) = default_source_by_id(&default_source_id) {
                seen_default_ids.insert(default_source_id);
                if reset_all || !source.user_modified {
                    save_source_to_db(
                        conn,
                        SourceConfig {
                            id: source.id,
                            enabled: if restore_hidden || reset_all {
                                true
                            } else {
                                source.enabled
                            },
                            hidden: if restore_hidden || reset_all {
                                false
                            } else {
                                source.hidden
                            },
                            is_deleted: if restore_hidden || reset_all {
                                false
                            } else {
                                source.is_deleted
                            },
                            deleted_at: if restore_hidden || reset_all {
                                None
                            } else {
                                source.deleted_at
                            },
                            created_at: source.created_at,
                            updated_at: source.updated_at,
                            ..default_source
                        },
                    )?;
                } else if restore_hidden && (source.hidden || source.is_deleted) {
                    save_source_to_db(
                        conn,
                        SourceConfig {
                            hidden: false,
                            is_deleted: false,
                            deleted_at: None,
                            enabled: true,
                            ..source
                        },
                    )?;
                } else if !source.is_default {
                    save_source_to_db(
                        conn,
                        SourceConfig {
                            is_default: true,
                            ..source
                        },
                    )?;
                }
            }
        }
    }

    for default_source in sample_sources() {
        let Some(default_source_id) = default_source.default_source_id.clone() else {
            continue;
        };
        if seen_default_ids.contains(&default_source_id) {
            continue;
        }
        if deleted_default_ids.contains(&default_source_id) {
            continue;
        }
        save_source_to_db(conn, default_source)?;
    }

    Ok(())
}

fn migrate_legacy_deleted_source(source: SourceConfig) -> SourceConfig {
    if source.hidden && (source.is_default || source.default_source_id.is_some()) {
        SourceConfig {
            hidden: false,
            is_deleted: true,
            deleted_at: source
                .deleted_at
                .clone()
                .or_else(|| Some(Utc::now().to_rfc3339())),
            user_modified: true,
            ..source
        }
    } else {
        source
    }
}

fn remember_deleted_default_source(
    conn: &Connection,
    default_source_id: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO deleted_default_sources (default_source_id, deleted_at)
         VALUES (?1, ?2)",
        params![default_source_id, Utc::now().to_rfc3339()],
    )
    .map_err(|error| format!("Could not remember deleted default source: {error}"))?;
    Ok(())
}

fn read_deleted_default_source_ids(conn: &Connection) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare("SELECT default_source_id FROM deleted_default_sources")
        .map_err(|error| format!("Could not read deleted default sources: {error}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Could not read deleted default sources: {error}"))?;
    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(|error| format!("Could not collect deleted default sources: {error}"))
}

async fn search_single_source(
    window: tauri::Window,
    source: SourceConfig,
    query: String,
) -> SourceSearchOutcome {
    let started = Instant::now();
    let search_url = build_search_url(&source, &query);
    let parser_mode = normalized_parser_mode(
        &source.parser_mode,
        &source.source_type,
        source.requires_javascript,
    );
    let mut debug_info = SourceDebugInfo {
        generated_search_url: Some(search_url.clone()),
        final_loaded_url: Some(search_url.clone()),
        parser_mode_used: Some(parser_mode.clone()),
        javascript_probably_required: Some(source.requires_javascript),
        browser_preview_limited: Some(false),
        ..SourceDebugInfo::default()
    };

    if source.source_type == "webviewOnly" || parser_mode == "fallbackOnly" {
        debug_info.final_action = Some("search fallback".to_string());
        debug_info.why_auto_open_failed =
            Some("Source is configured for fallback-only/WebView search.".to_string());
        return outcome_with_debug(
            &source,
            "ready",
            Some("WebView-only source. Open source search page.".to_string()),
            started,
            vec![webview_result(&source, &query, &search_url)],
            debug_info,
        );
    }
    if source.result_open_behavior == "search_page" {
        debug_info.final_action = Some("search fallback".to_string());
        debug_info.why_auto_open_failed =
            Some("Source is configured to open the search page.".to_string());
        return outcome_with_debug(
            &source,
            "ready",
            Some("Configured to open the source search page.".to_string()),
            started,
            vec![webview_result(&source, &query, &search_url)],
            debug_info,
        );
    }

    if let Err(error) = validate_source(&source) {
        debug_info.final_action = Some("failed".to_string());
        debug_info.timeout_or_error = Some(error.clone());
        return outcome_with_debug(&source, "error", Some(error), started, Vec::new(), debug_info);
    }

    if source.method.to_uppercase() != "GET" {
        debug_info.final_action = Some("failed".to_string());
        debug_info.timeout_or_error = Some("Only GET source searches are supported.".to_string());
        return outcome_with_debug(
            &source,
            "error",
            Some("Only GET source searches are supported in v1.".to_string()),
            started,
            Vec::new(),
            debug_info,
        );
    }

    if source.source_type == "directPage" {
        debug_info.final_action = Some("exact page".to_string());
        debug_info.best_score = Some(100.0);
        return outcome_with_debug(
            &source,
            "found",
            Some("Direct page source. Opening configured page.".to_string()),
            started,
            vec![direct_page_result(&source, &query, &search_url)],
            debug_info,
        );
    }

    let headers = match build_headers(&source.headers) {
        Ok(headers) => headers,
        Err(error) => {
            debug_info.final_action = Some("failed".to_string());
            debug_info.timeout_or_error = Some(error.clone());
            return outcome_with_debug(&source, "error", Some(error), started, Vec::new(), debug_info);
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(source_timeout_ms(&source)))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            let message = format!("Could not create HTTP client: {error}");
            debug_info.final_action = Some("failed".to_string());
            debug_info.timeout_or_error = Some(message.clone());
            return outcome_with_debug(
                &source,
                "error",
                Some(message),
                started,
                Vec::new(),
                debug_info,
            )
        }
    };

    let mut candidates = Vec::new();
    let mut static_error: Option<String> = None;
    if parser_mode != "webview" {
        match fetch_search_html_with_retries(&client, &headers, &source, &search_url).await {
            Ok(html) => {
                debug_info.static_fetch_worked = Some(true);
                debug_info.html_length = Some(html.len());
                match parse_results(&source, &query, &search_url, &html) {
                    Ok(parsed) => {
                        debug_info.static_parse_worked = Some(!parsed.is_empty());
                        update_debug_candidates(&mut debug_info, &parsed);
                        candidates = parsed;
                    }
                    Err(error) => {
                        debug_info.static_parse_worked = Some(false);
                        debug_info.timeout_or_error = Some(error.clone());
                        static_error = Some(error);
                    }
                }
            }
            Err(FetchFailure::TimedOut(message)) => {
                debug_info.static_fetch_worked = Some(false);
                debug_info.timeout_or_error = Some(message.clone());
                static_error = Some(message);
            }
            Err(FetchFailure::SelectorMissing(message)) => {
                debug_info.static_fetch_worked = Some(true);
                debug_info.static_parse_worked = Some(false);
                debug_info.why_auto_open_failed = Some(message.clone());
                static_error = Some(message);
            }
            Err(FetchFailure::Failed(message)) => {
                debug_info.static_fetch_worked = Some(false);
                debug_info.timeout_or_error = Some(message.clone());
                static_error = Some(message);
            }
        }
    }

    if parser_mode == "webview" || (parser_mode == "hybrid" && candidates.is_empty()) {
        match parse_search_with_webview(&window, &source, &query, &search_url).await {
            Ok(rendered) => {
                debug_info.web_view_parse_worked = Some(!rendered.results.is_empty());
                debug_info.final_loaded_url = Some(rendered.final_url);
                if debug_info.html_length.unwrap_or(0) < rendered.html_length {
                    debug_info.html_length = Some(rendered.html_length);
                }
                if !rendered.results.is_empty() {
                    update_debug_candidates(&mut debug_info, &rendered.results);
                    candidates = rendered.results;
                }
            }
            Err(error) => {
                debug_info.web_view_parse_worked = Some(false);
                debug_info.timeout_or_error = Some(error.clone());
                if static_error.is_none() {
                    static_error = Some(error);
                }
            }
        }
    }

    if candidates.is_empty() {
        let message = static_error
            .clone()
            .unwrap_or_else(|| "No parsed results. Open source search page.".to_string());
        debug_info.final_action = Some(if parser_mode == "static" && debug_info.static_fetch_worked == Some(false) {
            "failed".to_string()
        } else {
            "search fallback".to_string()
        });
        debug_info.why_auto_open_failed =
            Some("No result candidates matched the configured or common selectors.".to_string());
        if parser_mode == "static" && debug_info.static_fetch_worked == Some(false) {
            return outcome_with_debug(
                &source,
                "error",
                Some(message),
                started,
                Vec::new(),
                debug_info,
            );
        }
        return outcome_with_debug(
            &source,
            "ready",
            Some(format!("{message} Open source search page.")),
            started,
            vec![webview_result(&source, &query, &search_url)],
            debug_info,
        );
    }

    let decision = decide_resolution(&source, &query, candidates);
    if decision.fallback_to_search || decision.results.is_empty() {
        debug_info.final_action = Some("search fallback".to_string());
        debug_info.why_auto_open_failed = Some(decision.reason.clone());
        return outcome_with_debug(
            &source,
            "ready",
            Some(decision.reason),
            started,
            vec![webview_result(&source, &query, &search_url)],
            debug_info,
        );
    }

    let results =
        resolve_playable_results(&client, &headers, &source, &search_url, decision.results).await;
    if results.is_empty() {
        debug_info.final_action = Some("search fallback".to_string());
        debug_info.why_auto_open_failed =
            Some("Parsed results were empty after target resolution.".to_string());
        outcome_with_debug(
            &source,
            "ready",
            Some("No parsed results. Open source search page.".to_string()),
            started,
            vec![webview_result(&source, &query, &search_url)],
            debug_info,
        )
    } else {
        debug_info.final_action = Some(if decision.ambiguous {
            "show choices".to_string()
        } else {
            "exact page".to_string()
        });
        outcome_with_debug(
            &source,
            "found",
            Some(decision.reason),
            started,
            results,
            debug_info,
        )
    }
}

async fn test_direct_source(
    source: SourceConfig,
    query: String,
    started: Instant,
) -> SourceTestResult {
    let final_search_url = build_search_url(&source, &query);
    if source.source_type == "webviewOnly" {
        return SourceTestResult {
            ok: true,
            message: "WebView-only source. Searches use a fallback provider card.".to_string(),
            result_count: 1,
            elapsed_ms: started.elapsed().as_millis(),
            final_search_url: Some(final_search_url.clone()),
            raw_status: Some("loaded".to_string()),
            selector_match_count: 0,
            preview_results: vec![SourcePreviewResult {
                title: format!("Provider card: {}", source.name),
                url: final_search_url,
                year: None,
                score: None,
                confidence_reason: None,
            }],
            fallback_used: true,
            best_match: None,
            final_open_url: Some(build_search_url(&source, &query)),
            query_specificity: None,
            ambiguous: false,
            detected_selectors: Vec::new(),
            debug_info: None,
        };
    }
    if source.result_open_behavior == "search_page" {
        return SourceTestResult {
            ok: true,
            message: "Configured to open source search page.".to_string(),
            result_count: 1,
            elapsed_ms: started.elapsed().as_millis(),
            final_search_url: Some(final_search_url.clone()),
            raw_status: Some("loaded".to_string()),
            selector_match_count: 0,
            preview_results: vec![SourcePreviewResult {
                title: format!("Provider card: {}", source.name),
                url: final_search_url.clone(),
                year: None,
                score: None,
                confidence_reason: None,
            }],
            fallback_used: true,
            best_match: None,
            final_open_url: Some(final_search_url),
            query_specificity: None,
            ambiguous: false,
            detected_selectors: Vec::new(),
            debug_info: None,
        };
    }
    if source.source_type == "directPage" {
        return SourceTestResult {
            ok: true,
            message: "Direct page source. Opens configured page.".to_string(),
            result_count: 1,
            elapsed_ms: started.elapsed().as_millis(),
            final_search_url: Some(final_search_url.clone()),
            raw_status: Some("loaded".to_string()),
            selector_match_count: 1,
            preview_results: vec![SourcePreviewResult {
                title: source.name.clone(),
                url: final_search_url.clone(),
                year: None,
                score: Some(100.0),
                confidence_reason: Some("Configured direct page.".to_string()),
            }],
            fallback_used: false,
            best_match: Some(SourcePreviewResult {
                title: source.name.clone(),
                url: final_search_url.clone(),
                year: None,
                score: Some(100.0),
                confidence_reason: Some("Configured direct page.".to_string()),
            }),
            final_open_url: Some(final_search_url),
            query_specificity: Some("Direct page source.".to_string()),
            ambiguous: false,
            detected_selectors: Vec::new(),
            debug_info: None,
        };
    }
    let headers = match build_headers(&source.headers) {
        Ok(headers) => headers,
        Err(error) => {
            return SourceTestResult {
                ok: false,
                message: error,
                result_count: 0,
                elapsed_ms: started.elapsed().as_millis(),
                final_search_url: Some(final_search_url.clone()),
                raw_status: Some("failed".to_string()),
                selector_match_count: 0,
                preview_results: Vec::new(),
                fallback_used: false,
                best_match: None,
                final_open_url: Some(final_search_url),
                query_specificity: None,
                ambiguous: false,
                detected_selectors: Vec::new(),
                debug_info: None,
            }
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(source_timeout_ms(&source)))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return SourceTestResult {
                ok: false,
                message: format!("Could not create HTTP client: {error}"),
                result_count: 0,
                elapsed_ms: started.elapsed().as_millis(),
                final_search_url: Some(final_search_url.clone()),
                raw_status: Some("failed".to_string()),
                selector_match_count: 0,
                preview_results: Vec::new(),
                fallback_used: false,
                best_match: None,
                final_open_url: Some(final_search_url),
                query_specificity: None,
                ambiguous: false,
                detected_selectors: Vec::new(),
                debug_info: None,
            }
        }
    };

    match fetch_search_html_with_retries(&client, &headers, &source, &final_search_url).await {
        Ok(html) => {
            let selector_match_count = selector_match_count(&source, &html).unwrap_or(0);
            let detected_selectors = detect_selector_candidates(&html);
            let parsed_results = parse_results(&source, &query, &final_search_url, &html)
                .unwrap_or_default();
            let parsed_count = parsed_results.len();
            let decision = decide_resolution(&source, &query, parsed_results.clone());
            let selected_result = decision
                .selected
                .clone()
                .or_else(|| decision.results.first().cloned());
            let watch_resolved_url = if !decision.fallback_to_search {
                if let Some(selected) = selected_result.as_ref() {
                    resolve_watch_url(&client, &headers, &source, &selected.url).await
                } else {
                    None
                }
            } else {
                None
            };
            let final_open_url = if decision.fallback_to_search {
                Some(final_search_url.clone())
            } else {
                watch_resolved_url
                    .clone()
                    .or_else(|| selected_result.as_ref().map(|result| result.url.clone()))
                    .or_else(|| Some(final_search_url.clone()))
            };
            let best_match = selected_result.as_ref().map(|result| SourcePreviewResult {
                title: result.title.clone(),
                url: result.url.clone(),
                year: result.year.clone(),
                score: Some(result.confidence),
                confidence_reason: result.raw_data.get("confidenceReason").cloned(),
            });
            let preview_source = if decision.results.is_empty() {
                parsed_results
            } else {
                decision.results.clone()
            };
            let preview_results = preview_source
                .into_iter()
                .take(5)
                .map(|result| {
                    let confidence_reason = result.raw_data.get("confidenceReason").cloned();
                    SourcePreviewResult {
                        title: result.title,
                        url: result.url,
                        year: result.year,
                        score: Some(result.confidence),
                        confidence_reason,
                    }
                })
                .collect::<Vec<_>>();
            let mut message = if parsed_count == 0 {
                "Source loaded, but no matching result cards were parsed.".to_string()
            } else {
                decision.reason.clone()
            };
            if watch_resolved_url.is_some() {
                message.push_str(" Watch/player page resolved.");
            }

            SourceTestResult {
                ok: true,
                message,
                result_count: preview_results.len(),
                elapsed_ms: started.elapsed().as_millis(),
                final_search_url: Some(final_search_url.clone()),
                raw_status: Some("loaded".to_string()),
                selector_match_count,
                preview_results: preview_results.clone(),
                fallback_used: decision.fallback_to_search || parsed_count == 0,
                final_open_url,
                best_match,
                query_specificity: Some(decision.query_specificity),
                ambiguous: decision.ambiguous,
                detected_selectors,
                debug_info: Some({
                    let mut debug = SourceDebugInfo {
                        generated_search_url: Some(final_search_url.clone()),
                        final_loaded_url: Some(final_search_url.clone()),
                        parser_mode_used: Some(normalized_parser_mode(
                            &source.parser_mode,
                            &source.source_type,
                            source.requires_javascript,
                        )),
                        static_fetch_worked: Some(true),
                        static_parse_worked: Some(parsed_count > 0),
                        web_view_parse_worked: None,
                        html_length: Some(html.len()),
                        javascript_probably_required: Some(source.requires_javascript),
                        final_action: Some(if decision.fallback_to_search {
                            "search fallback".to_string()
                        } else if decision.ambiguous {
                            "show choices".to_string()
                        } else {
                            "exact page".to_string()
                        }),
                        ..SourceDebugInfo::default()
                    };
                    if decision.fallback_to_search {
                        debug.why_auto_open_failed = Some(decision.reason.clone());
                    }
                    debug.result_container_count = Some(parsed_count);
                    debug.candidate_titles = preview_results
                        .iter()
                        .take(5)
                        .map(|result| result.title.clone())
                        .collect();
                    debug.candidate_links = preview_results
                        .iter()
                        .take(5)
                        .map(|result| result.url.clone())
                        .collect();
                    debug.best_score = preview_results
                        .iter()
                        .filter_map(|result| result.score)
                        .next();
                    debug
                }),
            }
        }
        Err(FetchFailure::TimedOut(message)) => SourceTestResult {
            ok: false,
            message,
            result_count: 0,
            elapsed_ms: started.elapsed().as_millis(),
            final_search_url: Some(final_search_url.clone()),
            raw_status: Some("timed out".to_string()),
            selector_match_count: 0,
            preview_results: Vec::new(),
            fallback_used: false,
            best_match: None,
            final_open_url: None,
            query_specificity: None,
            ambiguous: false,
            detected_selectors: Vec::new(),
            debug_info: None,
        },
        Err(FetchFailure::SelectorMissing(message)) => SourceTestResult {
            ok: false,
            message,
            result_count: 0,
            elapsed_ms: started.elapsed().as_millis(),
            final_search_url: Some(final_search_url.clone()),
            raw_status: Some("loaded".to_string()),
            selector_match_count: 0,
            preview_results: Vec::new(),
            fallback_used: true,
            best_match: None,
            final_open_url: Some(final_search_url),
            query_specificity: None,
            ambiguous: false,
            detected_selectors: Vec::new(),
            debug_info: None,
        },
        Err(FetchFailure::Failed(message)) => SourceTestResult {
            ok: false,
            message,
            result_count: 0,
            elapsed_ms: started.elapsed().as_millis(),
            final_search_url: Some(final_search_url.clone()),
            raw_status: Some("failed".to_string()),
            selector_match_count: 0,
            preview_results: Vec::new(),
            fallback_used: true,
            best_match: None,
            final_open_url: Some(final_search_url),
            query_specificity: None,
            ambiguous: false,
            detected_selectors: Vec::new(),
            debug_info: None,
        },
    }
}

enum FetchFailure {
    TimedOut(String),
    SelectorMissing(String),
    Failed(String),
}

async fn fetch_search_html_with_retries(
    client: &reqwest::Client,
    headers: &HeaderMap,
    source: &SourceConfig,
    search_url: &str,
) -> Result<String, FetchFailure> {
    let max_retries = source.max_retries.min(5);
    let mut last_failure = FetchFailure::Failed("Source did not respond.".to_string());

    for attempt in 0..=max_retries {
        match fetch_search_html_once(client, headers, source, search_url).await {
            Ok(html) => return Ok(html),
            Err(error) => last_failure = error,
        }

        if attempt < max_retries {
            sleep(Duration::from_millis(700 + u64::from(attempt) * 400)).await;
        }
    }

    Err(match last_failure {
        FetchFailure::SelectorMissing(_) => FetchFailure::SelectorMissing(
            "No results or selector not found after retry.".to_string(),
        ),
        FetchFailure::TimedOut(_) => FetchFailure::TimedOut(format!(
            "Timed out after {}ms and {} retr{}.",
            source_timeout_ms(source),
            max_retries,
            if max_retries == 1 { "y" } else { "ies" }
        )),
        FetchFailure::Failed(message) => FetchFailure::Failed(format!(
            "{message} Error after retry."
        )),
    })
}

async fn fetch_search_html_once(
    client: &reqwest::Client,
    headers: &HeaderMap,
    source: &SourceConfig,
    search_url: &str,
) -> Result<String, FetchFailure> {
    let response = client
        .get(search_url)
        .headers(headers.clone())
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                FetchFailure::TimedOut(format!("Timed out after {}ms.", source_timeout_ms(source)))
            } else {
                FetchFailure::Failed(format!("Network error: {error}"))
            }
        })?;

    if !response.status().is_success() {
        return Err(FetchFailure::Failed(format!("HTTP error: {}", response.status())));
    }

    let html = response.text().await.map_err(|error| {
        if error.is_timeout() {
            FetchFailure::TimedOut(format!("Timed out after {}ms.", source_timeout_ms(source)))
        } else {
            FetchFailure::Failed(format!("Could not read response body: {error}"))
        }
    })?;

    let delay_ms = source.load_delay_ms.min(10_000);
    if delay_ms > 0 {
        sleep(Duration::from_millis(delay_ms)).await;
    }

    let wait_selector = source
        .wait_for_selector
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&source.result_selector);
    if !wait_selector.trim().is_empty() && selector_match_count_for(wait_selector, &html)? == 0 {
        return Err(FetchFailure::SelectorMissing(
            "No results or selector not found.".to_string(),
        ));
    }

    Ok(html)
}

async fn resolve_playable_results(
    client: &reqwest::Client,
    headers: &HeaderMap,
    source: &SourceConfig,
    _search_url: &str,
    candidates: Vec<SearchResult>,
) -> Vec<SearchResult> {
    let tasks = candidates
        .into_iter()
        .take(30)
        .map(|mut result| async move {
            if result.playable_url.is_some() {
                return Some(result);
            }

            if let Some(watch_url) =
                resolve_watch_url(client, headers, source, &result.url).await
            {
                result
                    .raw_data
                    .insert("detailPageUrl".to_string(), result.url.clone());
                result
                    .raw_data
                    .insert("openedVia".to_string(), "watchButtonSelector".to_string());
                result.url = watch_url;
            }

            let Some(video_selector) = source.video_selector.as_deref() else {
                return Some(SearchResult {
                    open_mode: Some("webview".to_string()),
                    ..result
                });
            };
            let selector = match Selector::parse(video_selector.trim()) {
                Ok(selector) => selector,
                Err(_) => {
                    return Some(SearchResult {
                        open_mode: Some("webview".to_string()),
                        ..result
                    })
                }
            };
            let video_attribute = source
                .video_attribute
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("src");

            let response = match client
                .get(&result.url)
                .headers(headers.clone())
                .send()
                .await
            {
                Ok(response) => response,
                Err(_) => {
                    return Some(SearchResult {
                        open_mode: Some("webview".to_string()),
                        ..result
                    })
                }
            };
            if !response.status().is_success() {
                return Some(SearchResult {
                    open_mode: Some("webview".to_string()),
                    ..result
                });
            }
            let html = match response.text().await {
                Ok(html) => html,
                Err(_) => {
                    return Some(SearchResult {
                        open_mode: Some("webview".to_string()),
                        ..result
                    })
                }
            };
            let document = Html::parse_document(&html);
            let Some(playable_url) = document
                .select(&selector)
                .next()
                .and_then(|node| node.value().attr(video_attribute))
                .and_then(|value| absolutize_url(&source.base_url, &result.url, value))
            else {
                return Some(SearchResult {
                    open_mode: Some("webview".to_string()),
                    ..result
                });
            };

            Some(SearchResult {
                open_mode: Some("native".to_string()),
                playable_url: Some(playable_url),
                ..result
            })
        });

    join_all(tasks)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
}

async fn resolve_watch_url(
    client: &reqwest::Client,
    headers: &HeaderMap,
    source: &SourceConfig,
    result_url: &str,
) -> Option<String> {
    let can_auto_resolve = source.auto_resolve_watch_page
        && (source.watch_button_selector.is_some()
        || source.auto_open_watch_button
        || source.auto_open_first_watch_link);
    if !can_auto_resolve {
        return None;
    }

    let mut current_url = result_url.to_string();
    for _step in 0..source.max_resolve_steps.min(5) {
        let response = client
            .get(&current_url)
            .headers(headers.clone())
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return if current_url == result_url { None } else { Some(current_url) };
        }
        let html = response.text().await.ok()?;
        let document = Html::parse_document(&html);
        if let Some(player_selector) = source.player_selector.as_deref() {
            if Selector::parse(player_selector)
                .ok()
                .map(|selector| document.select(&selector).next().is_some())
                .unwrap_or(false)
            {
                return if current_url == result_url { None } else { Some(current_url) };
            }
        }
        let Some(next_url) = find_watch_url(source, &document, &current_url) else {
            return if current_url == result_url { None } else { Some(current_url) };
        };
        if next_url == current_url {
            return if current_url == result_url { None } else { Some(current_url) };
        }
        current_url = next_url;
    }

    if current_url == result_url {
        None
    } else {
        Some(current_url)
    }
}

fn find_watch_url(source: &SourceConfig, document: &Html, page_url: &str) -> Option<String> {
    if let Some(selector_text) = source.watch_button_selector.as_deref() {
        if let Ok(selector) = Selector::parse(selector_text.trim()) {
            for node in document.select(&selector) {
                if let Some(url) = node_url(source, page_url, node) {
                    return Some(url);
                }
            }
        }
    }

    if !source.auto_open_watch_button && !source.auto_open_first_watch_link {
        return None;
    }

    let normalized_patterns = source
        .watch_link_text_patterns
        .iter()
        .map(|pattern| normalize_comparable_title(pattern))
        .collect::<Vec<_>>();
    let selector = Selector::parse("a[href], button, [role='button'], [data-href], [data-url]").ok()?;
    for node in document.select(&selector) {
        let mut text_parts = vec![collapse_whitespace(&node.text().collect::<Vec<_>>().join(" "))];
        if let Some(title) = node.value().attr("title") {
            text_parts.push(title.to_string());
        }
        if let Some(label) = node.value().attr("aria-label") {
            text_parts.push(label.to_string());
        }
        let text = normalize_comparable_title(&text_parts.join(" "));
        if normalized_patterns
            .iter()
            .any(|pattern| !pattern.is_empty() && text.contains(pattern))
        {
            if let Some(url) = node_url(source, page_url, node) {
                return Some(url);
            }
        }
    }

    None
}

fn node_url(source: &SourceConfig, page_url: &str, node: ElementRef<'_>) -> Option<String> {
    node.value()
        .attr("href")
        .or_else(|| node.value().attr("data-href"))
        .or_else(|| node.value().attr("data-url"))
        .and_then(|value| absolutize_url(&source.base_url, page_url, value))
}

fn parse_results(
    source: &SourceConfig,
    query: &str,
    search_url: &str,
    html: &str,
) -> Result<Vec<SearchResult>, String> {
    let document = Html::parse_document(html);
    let mut best_results = Vec::new();
    let mut best_score = 0.0;
    for selector_text in result_selector_candidates(source) {
        let results =
            parse_results_with_selector(source, query, search_url, &document, &selector_text)?;
        let strongest_match = results
            .iter()
            .map(|result| result.confidence)
            .fold(0.0, f64::max);
        let selector_score = strongest_match + results.len().min(12) as f64;
        if selector_score > best_score {
            best_score = selector_score;
            best_results = results;
        }
    }

    Ok(best_results
        .into_iter()
        .filter(|result| result.confidence >= 12.0)
        .take(24)
        .collect())
}

fn parse_results_with_selector(
    source: &SourceConfig,
    query: &str,
    search_url: &str,
    document: &Html,
    result_selector_text: &str,
) -> Result<Vec<SearchResult>, String> {
    let result_selector = parse_required_selector("resultSelector", result_selector_text)?;
    let title_selector = parse_optional_selector("titleSelector", &source.title_selector)?;
    let poster_selector = parse_optional_selector("posterSelector", &source.poster_selector)?;
    let link_selector = parse_optional_selector("linkSelector", &source.link_selector)?;
    let video_selector = parse_optional_selector("videoSelector", &source.video_selector)?;
    let year_selector = parse_optional_selector("yearSelector", &source.year_selector)?;
    let description_selector =
        parse_optional_selector("descriptionSelector", &source.description_selector)?;
    let poster_attribute = source
        .poster_attribute
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("src");
    let link_attribute = source
        .link_attribute
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("href");
    let video_attribute = source
        .video_attribute
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("src");

    let mut seen = HashSet::new();
    let mut results = Vec::new();

    for (index, node) in document.select(&result_selector).enumerate().take(80) {
        let raw_link = link_selector
            .as_ref()
            .and_then(|selector| first_attr(node, selector, link_attribute))
            .or_else(|| first_attr_any(node, common_link_selectors(), link_attribute))
            .or_else(|| node.value().attr(link_attribute).map(|value| value.to_string()));

        let Some(raw_link) = raw_link else {
            continue;
        };

        let Some(url) = absolutize_url(&source.base_url, search_url, &raw_link) else {
            continue;
        };

        let title = title_selector
            .as_ref()
            .and_then(|selector| first_text(node, selector))
            .or_else(|| first_text_any(node, common_title_selectors()))
            .or_else(|| node.value().attr("title").map(str::to_string))
            .or_else(|| node.value().attr("aria-label").map(str::to_string))
            .or_else(|| first_attr_any(node, &["img"], "alt"))
            .or_else(|| Some(collapse_whitespace(&node.text().collect::<Vec<_>>().join(" "))))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| url.clone());

        let year = year_selector
            .as_ref()
            .and_then(|selector| first_text(node, selector))
            .or_else(|| first_text_any(node, common_year_selectors()))
            .or_else(|| extract_year(&title))
            .filter(|value| !value.is_empty());
        let description = description_selector
            .as_ref()
            .and_then(|selector| first_text(node, selector))
            .filter(|value| !value.is_empty());
        let poster_url = poster_selector
            .as_ref()
            .and_then(|selector| first_poster_attr(node, selector, poster_attribute))
            .or_else(|| first_poster_attr_any(node, common_poster_selectors(), poster_attribute))
            .and_then(|value| absolutize_url(&source.base_url, search_url, &value));
        let playable_url = video_selector
            .as_ref()
            .and_then(|selector| first_attr(node, selector, video_attribute))
            .and_then(|value| absolutize_url(&source.base_url, search_url, &value))
            .or_else(|| {
                if is_playable_video_url(&url) {
                    Some(url.clone())
                } else {
                    None
                }
            });

        let dedupe_key = format!(
            "{}:{}:{}",
            source.id,
            normalize_match_key(&title),
            year.clone().unwrap_or_default()
        );
        if !seen.insert(dedupe_key) {
            continue;
        }

        let mut raw_data = HashMap::new();
        raw_data.insert("resultIndex".to_string(), index.to_string());
        raw_data.insert("rawLink".to_string(), raw_link);
        raw_data.insert("resultSelector".to_string(), result_selector_text.to_string());

        let scored = score_result_candidate(query, &title, year.as_deref());
        let confidence = scored.score;
        let quality = quality_for_score(confidence, "parsed");
        results.push(SearchResult {
            id: format!("{}-{index}", source.id),
            source_id: source.id.clone(),
            source_name: source.name.clone(),
            title,
            url,
            open_mode: Some(if playable_url.is_some() {
                "native".to_string()
            } else {
                "webview".to_string()
            }),
            playable_url,
            poster_url,
            year,
            description,
            confidence,
            quality: Some(quality.clone()),
            reason: Some(scored.reason.clone()),
            source_reliability: Some(source_reliability(source)),
            debug_info: None,
            raw_data: {
                raw_data.insert("confidenceReason".to_string(), scored.reason);
                raw_data.insert("quality".to_string(), quality);
                raw_data
            },
        });
    }

    results.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

struct ResolverDecision {
    fallback_to_search: bool,
    results: Vec<SearchResult>,
    reason: String,
    ambiguous: bool,
    query_specificity: String,
    selected: Option<SearchResult>,
}

fn decide_resolution(
    source: &SourceConfig,
    query: &str,
    candidates: Vec<SearchResult>,
) -> ResolverDecision {
    let specificity = is_specific_query(query);
    let mut ranked = candidates
        .into_iter()
        .map(|mut candidate| {
            let scored = score_result_candidate(query, &candidate.title, candidate.year.as_deref());
            candidate.confidence = scored.score;
            let quality = quality_for_score(scored.score, "parsed");
            candidate.quality = Some(quality.clone());
            candidate.reason = Some(scored.reason.clone());
            candidate
                .raw_data
                .insert("confidenceReason".to_string(), scored.reason);
            candidate.raw_data.insert("quality".to_string(), quality);
            candidate
        })
        .filter(|candidate| candidate.confidence >= 35.0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let Some(best) = ranked.first().cloned() else {
        return ResolverDecision {
            fallback_to_search: true,
            results: Vec::new(),
            reason: "Could not parse exact result. Open source search page.".to_string(),
            ambiguous: false,
            query_specificity: specificity.reason,
            selected: None,
        };
    };

    let close_second = ranked
        .get(1)
        .map(|second| best.confidence - second.confidence <= 8.0 && second.confidence >= 70.0)
        .unwrap_or(false);
    let query_year = extract_year(query);
    let best_year = best.year.as_deref().and_then(extract_year);
    let year_mismatch = query_year.is_some()
        && best_year.is_some()
        && query_year.as_deref() != best_year.as_deref();
    let threshold = source.exact_match_threshold as f64;
    let ambiguous = !specificity.specific
        || close_second
        || best.confidence < threshold
        || year_mismatch
        || !source.auto_open_best_match;

    if ambiguous {
        if source.ambiguous_query_behavior == "open_search_page" {
            return ResolverDecision {
                fallback_to_search: true,
                results: Vec::new(),
                reason: if year_mismatch {
                    "Best result year does not match query year. Open source search page.".to_string()
                } else {
                    "Multiple possible matches. Open source search page.".to_string()
                },
                ambiguous: true,
                query_specificity: specificity.reason,
                selected: Some(best),
            };
        }

        let choices = ranked
            .into_iter()
            .take(5)
            .map(|mut result| {
                result
                    .raw_data
                    .insert("resolution".to_string(), "ambiguous".to_string());
                result.raw_data.insert("quality".to_string(), "medium".to_string());
                result
                    .raw_data
                    .insert("querySpecificity".to_string(), specificity.reason.clone());
                result.quality = Some("medium".to_string());
                result.reason = Some("Multiple possible matches".to_string());
                result
            })
            .collect::<Vec<_>>();
        return ResolverDecision {
            fallback_to_search: false,
            results: choices,
            reason: if year_mismatch {
                "Multiple possible matches; best year does not match the query.".to_string()
            } else {
                format!("Multiple possible matches. {}", specificity.reason)
            },
            ambiguous: true,
            query_specificity: specificity.reason,
            selected: Some(best),
        };
    }

    let mut selected = best;
    selected
        .raw_data
        .insert("resolution".to_string(), "exact".to_string());
    let selected_quality = quality_for_score(selected.confidence, "exact");
    selected
        .raw_data
        .insert("quality".to_string(), selected_quality.clone());
    selected
        .raw_data
        .insert("querySpecificity".to_string(), specificity.reason.clone());
    selected.quality = Some(selected_quality);
    selected.reason = selected.raw_data.get("confidenceReason").cloned();
    ResolverDecision {
        fallback_to_search: false,
        reason: format!(
            "Movie page found. {}",
            selected
                .raw_data
                .get("confidenceReason")
                .cloned()
                .unwrap_or_else(|| "Strong title match.".to_string())
        ),
        results: vec![selected.clone()],
        ambiguous: false,
        query_specificity: specificity.reason,
        selected: Some(selected),
    }
}

struct QuerySpecificity {
    specific: bool,
    reason: String,
}

fn is_specific_query(query: &str) -> QuerySpecificity {
    let normalized = normalize_comparable_title(query);
    if extract_year(query).is_some() {
        return QuerySpecificity {
            specific: true,
            reason: "Query includes a year.".to_string(),
        };
    }
    let lower = query.to_lowercase();
    if lower.contains("season")
        || lower.contains("episode")
        || lower.split_whitespace().any(|token| {
            let token = token.trim_matches(|character: char| !character.is_alphanumeric());
            (token.starts_with('s') || token.starts_with('S'))
                && token.chars().skip(1).all(|character| character.is_ascii_digit())
        })
    {
        return QuerySpecificity {
            specific: true,
            reason: "Query includes season or episode detail.".to_string(),
        };
    }
    if broad_franchise_queries().contains(normalized.as_str()) {
        return QuerySpecificity {
            specific: false,
            reason: "Broad franchise query.".to_string(),
        };
    }
    if normalized.split_whitespace().count() >= 2 {
        return QuerySpecificity {
            specific: true,
            reason: "Query is a specific title phrase.".to_string(),
        };
    }
    QuerySpecificity {
        specific: false,
        reason: "Short broad query.".to_string(),
    }
}

fn broad_franchise_queries() -> HashSet<&'static str> {
    HashSet::from([
        "batman",
        "spider man",
        "spiderman",
        "superman",
        "x men",
        "xmen",
        "star wars",
        "star trek",
    ])
}

fn outcome_with_debug(
    source: &SourceConfig,
    status: &str,
    message: Option<String>,
    started: Instant,
    mut results: Vec<SearchResult>,
    debug_info: SourceDebugInfo,
) -> SourceSearchOutcome {
    for result in &mut results {
        result.debug_info = Some(debug_info.clone());
        if result.raw_data.get("resolution").map(String::as_str) == Some("fallback") {
            if let Ok(value) = serde_json::to_string(&debug_info) {
                result.raw_data.insert("fallbackDebug".to_string(), value);
            }
        }
    }
    SourceSearchOutcome {
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        status: status.to_string(),
        message,
        elapsed_ms: started.elapsed().as_millis(),
        results,
        debug_info: Some(debug_info),
    }
}

fn update_debug_candidates(debug_info: &mut SourceDebugInfo, candidates: &[SearchResult]) {
    debug_info.result_container_count = Some(candidates.len());
    debug_info.candidate_titles = candidates
        .iter()
        .take(5)
        .map(|candidate| candidate.title.clone())
        .collect();
    debug_info.candidate_links = candidates
        .iter()
        .take(5)
        .map(|candidate| candidate.url.clone())
        .collect();
    debug_info.best_score = candidates.first().map(|candidate| candidate.confidence);
}

fn compare_outcomes(left: &SourceSearchOutcome, right: &SourceSearchOutcome) -> std::cmp::Ordering {
    let left_tuple = outcome_sort_tuple(left);
    let right_tuple = outcome_sort_tuple(right);
    right_tuple
        .0
        .cmp(&left_tuple.0)
        .then_with(|| {
            right_tuple
                .1
                .partial_cmp(&left_tuple.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            right_tuple
                .2
                .partial_cmp(&left_tuple.2)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| left.source_name.cmp(&right.source_name))
}

fn outcome_sort_tuple(outcome: &SourceSearchOutcome) -> (i32, f64, f64) {
    let Some(best) = outcome.results.first() else {
        return (
            if outcome.status == "error" || outcome.status == "timed_out" {
                quality_rank("failed")
            } else {
                0
            },
            0.0,
            0.0,
        );
    };
    (
        quality_rank(best.quality.as_deref().unwrap_or_else(|| {
            best.raw_data
                .get("quality")
                .map(String::as_str)
                .unwrap_or("weak")
        })),
        best.confidence,
        best.source_reliability.unwrap_or(0.0),
    )
}

fn quality_rank(value: &str) -> i32 {
    match value {
        "excellent" => 5,
        "good" => 4,
        "medium" => 3,
        "weak" => 2,
        "failed" => 1,
        _ => 0,
    }
}

fn validate_source(source: &SourceConfig) -> Result<(), String> {
    if source.id.trim().is_empty() {
        return Err("Source id is required.".to_string());
    }
    if source.name.trim().is_empty() {
        return Err("Source name is required.".to_string());
    }
    Url::parse(source.base_url.trim()).map_err(|_| "Base URL must be an absolute URL.".to_string())?;
    if !source.source_type.eq_ignore_ascii_case("directPage")
        && !source.search_url.contains("{query}")
        && !source.search_url.contains("{slug}")
    {
        return Err("Search URL must include {query} or {slug}.".to_string());
    }
    if is_web_source(source) {
        return Ok(());
    }
    if source.result_selector.trim().is_empty() {
        return Ok(());
    }
    parse_required_selector("resultSelector", &source.result_selector).map(|_| ())
}

fn build_headers(headers: &HashMap<String, String>) -> Result<HeaderMap, String> {
    let mut header_map = HeaderMap::new();
    header_map.insert(
        USER_AGENT,
        HeaderValue::from_str(DEFAULT_USER_AGENT)
            .map_err(|error| format!("Invalid default user agent: {error}"))?,
    );

    for (name, value) in headers {
        if name.trim().is_empty() || value.trim().is_empty() {
            continue;
        }
        let header_name = HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|_| format!("Invalid header name: {name}"))?;
        let header_value = HeaderValue::from_str(value.trim())
            .map_err(|_| format!("Invalid header value for {name}"))?;
        header_map.insert(header_name, header_value);
    }

    Ok(header_map)
}

fn build_search_url(source: &SourceConfig, query: &str) -> String {
    let encoded_query = encode_query_component(query);
    let slug_query = encode_query_component(&query.split_whitespace().collect::<Vec<_>>().join("-"));
    let template = if source.search_url.trim().is_empty() {
        source.base_url.trim()
    } else {
        source.search_url.trim()
    };
    let raw = template
        .replace("{query}", &encoded_query)
        .replace("{slug}", &slug_query);
    if Url::parse(&raw).is_ok() {
        return raw;
    }
    Url::parse(source.base_url.trim())
        .ok()
        .and_then(|base| base.join(&raw).ok())
        .map(|url| url.to_string())
        .unwrap_or(raw)
}

fn source_timeout_ms(source: &SourceConfig) -> u64 {
    source.request_timeout_ms.clamp(3_000, 60_000)
}

fn webview_result(source: &SourceConfig, query: &str, url: &str) -> SearchResult {
    let mut raw_data = HashMap::new();
    raw_data.insert("provider".to_string(), "javascript-webview".to_string());
    raw_data.insert("resultKind".to_string(), "provider".to_string());
    raw_data.insert("primary".to_string(), "true".to_string());
    raw_data.insert("resolution".to_string(), "fallback".to_string());
    raw_data.insert("quality".to_string(), "weak".to_string());
    raw_data.insert("confidenceReason".to_string(), "Search fallback".to_string());

    SearchResult {
        id: stable_id(&format!("{}:{url}", source.id)),
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        title: format!("Search \"{query}\" on {}", source.name),
        url: url.to_string(),
        open_mode: Some("webview".to_string()),
        playable_url: None,
        poster_url: None,
        year: None,
        description: Some("Opens this JavaScript-heavy source in the in-app viewer.".to_string()),
        confidence: 0.0,
        quality: Some("weak".to_string()),
        reason: Some("Search fallback".to_string()),
        source_reliability: Some(source_reliability(source)),
        debug_info: None,
        raw_data,
    }
}

fn direct_page_result(source: &SourceConfig, query: &str, url: &str) -> SearchResult {
    let mut raw_data = HashMap::new();
    raw_data.insert("provider".to_string(), "direct-page".to_string());
    raw_data.insert("resultKind".to_string(), "parsed".to_string());
    raw_data.insert("primary".to_string(), "true".to_string());
    raw_data.insert("resolution".to_string(), "exact".to_string());
    raw_data.insert("quality".to_string(), "excellent".to_string());
    raw_data.insert("confidenceReason".to_string(), "Configured direct page.".to_string());
    let playable_url = if is_playable_video_url(url) {
        Some(url.to_string())
    } else {
        None
    };

    SearchResult {
        id: stable_id(&format!("{}:{url}", source.id)),
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        title: if source.name.trim().is_empty() {
            query.to_string()
        } else {
            source.name.clone()
        },
        url: url.to_string(),
        open_mode: Some(if playable_url.is_some() {
            "native".to_string()
        } else {
            "webview".to_string()
        }),
        playable_url,
        poster_url: None,
        year: None,
        description: Some("Configured direct page. Opens inside CineFinder.".to_string()),
        confidence: 100.0,
        quality: Some("excellent".to_string()),
        reason: Some("Configured direct page.".to_string()),
        source_reliability: Some(source_reliability(source)),
        debug_info: None,
        raw_data,
    }
}

fn quality_for_score(score: f64, resolution: &str) -> String {
    if resolution == "fallback" {
        return "weak".to_string();
    }
    if score >= 92.0 {
        "excellent".to_string()
    } else if score >= 85.0 {
        "good".to_string()
    } else if score >= 70.0 {
        "medium".to_string()
    } else {
        "weak".to_string()
    }
}

fn source_reliability(source: &SourceConfig) -> f64 {
    let mut score: f64 = 50.0;
    if source.is_default {
        score += 12.0;
    }
    if !source.result_selector.trim().is_empty() {
        score += 10.0;
    }
    match normalized_parser_mode(&source.parser_mode, &source.source_type, source.requires_javascript).as_str() {
        "hybrid" => score += 12.0,
        "static" | "webview" => score += 6.0,
        "fallbackOnly" => score -= 25.0,
        _ => {}
    }
    if source.result_open_behavior == "search_page" {
        score -= 25.0;
    }
    if source.user_modified {
        score += 3.0;
    }
    score.clamp(0.0, 100.0)
}

fn selector_match_count(source: &SourceConfig, html: &str) -> Result<usize, String> {
    if source.result_selector.trim().is_empty() {
        return Ok(common_result_selectors()
            .iter()
            .filter_map(|selector| selector_match_count_for(selector, html).ok())
            .max()
            .unwrap_or(0));
    }
    selector_match_count_for(&source.result_selector, html).map_err(|error| match error {
        FetchFailure::Failed(message)
        | FetchFailure::SelectorMissing(message)
        | FetchFailure::TimedOut(message) => message,
    })
}

fn detect_selector_candidates(html: &str) -> Vec<SelectorCandidate> {
    let document = Html::parse_document(html);
    let mut candidates = Vec::new();
    push_selector_candidates(&document, "result", common_result_selectors(), &mut candidates);
    push_selector_candidates(&document, "title", common_title_selectors(), &mut candidates);
    push_selector_candidates(&document, "poster", common_poster_selectors(), &mut candidates);
    candidates.sort_by(|left, right| right.match_count.cmp(&left.match_count));
    candidates.truncate(12);
    candidates
}

fn push_selector_candidates(
    document: &Html,
    selector_type: &str,
    selectors: &[&str],
    candidates: &mut Vec<SelectorCandidate>,
) {
    for selector_text in selectors {
        let Ok(selector) = Selector::parse(selector_text) else {
            continue;
        };
        let mut matches = document.select(&selector);
        let Some(first) = matches.next() else {
            continue;
        };
        let sample = collapse_whitespace(&first.text().collect::<Vec<_>>().join(" "));
        let match_count = 1 + matches.count();
        candidates.push(SelectorCandidate {
            selector_type: selector_type.to_string(),
            selector: selector_text.to_string(),
            match_count,
            sample: if sample.is_empty() {
                None
            } else {
                Some(sample.chars().take(80).collect())
            },
        });
    }
}

fn selector_match_count_for(selector: &str, html: &str) -> Result<usize, FetchFailure> {
    let selector = Selector::parse(selector.trim())
        .map_err(|_| FetchFailure::Failed(format!("Invalid CSS selector: {selector}")))?;
    let document = Html::parse_document(html);
    Ok(document.select(&selector).count())
}

fn parse_required_selector(label: &str, selector: &str) -> Result<Selector, String> {
    Selector::parse(selector.trim()).map_err(|_| format!("Invalid CSS selector in {label}."))
}

fn parse_optional_selector(
    label: &str,
    selector: &Option<String>,
) -> Result<Option<Selector>, String> {
    match selector.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Selector::parse(value)
            .map(Some)
            .map_err(|_| format!("Invalid CSS selector in {label}.")),
        None => Ok(None),
    }
}

fn result_selector_candidates(source: &SourceConfig) -> Vec<String> {
    if !source.result_selector.trim().is_empty() {
        return vec![source.result_selector.trim().to_string()];
    }
    common_result_selectors()
        .iter()
        .map(|selector| selector.to_string())
        .collect()
}

fn common_result_selectors() -> &'static [&'static str] {
    &[
        "article",
        ".card",
        ".movie",
        ".movie-card",
        ".film",
        ".item",
        ".post",
        ".entry",
        ".video",
        ".poster",
        ".thumb",
        ".result",
        ".ml-item",
        ".flw-item",
        ".film_list-wrap .flw-item",
        ".content .item",
        ".short",
        ".b-content__inline_item",
        "a[href]",
    ]
}

fn common_title_selectors() -> &'static [&'static str] {
    &[
        "h1",
        "h2",
        "h3",
        "h4",
        ".title",
        ".name",
        ".movie-title",
        ".film-title",
        ".entry-title",
        ".b-content__inline_item-link",
        ".short-title",
        "[title]",
        "img[alt]",
    ]
}

fn common_link_selectors() -> &'static [&'static str] {
    &[
        "a[href]",
        ".title a",
        ".movie-title a",
        ".poster a",
        ".thumb a",
        "article a",
        ".card a",
        ".b-content__inline_item-link",
        ".short-title a",
    ]
}

fn common_poster_selectors() -> &'static [&'static str] {
    &[
        "img",
        ".poster img",
        ".thumb img",
        "picture img",
        "img[data-src]",
        "img[data-original]",
        "img[data-lazy-src]",
    ]
}

fn common_year_selectors() -> &'static [&'static str] {
    &[".year", ".date", ".release"]
}

fn first_text(parent: ElementRef<'_>, selector: &Selector) -> Option<String> {
    parent
        .select(selector)
        .next()
        .map(|node| collapse_whitespace(&node.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty())
}

fn first_text_any(parent: ElementRef<'_>, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|selector_text| {
        Selector::parse(selector_text)
            .ok()
            .and_then(|selector| first_text(parent, &selector))
    })
}

fn first_attr(parent: ElementRef<'_>, selector: &Selector, attribute: &str) -> Option<String> {
    parent
        .select(selector)
        .next()
        .and_then(|node| node.value().attr(attribute))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn first_attr_any(parent: ElementRef<'_>, selectors: &[&str], attribute: &str) -> Option<String> {
    selectors.iter().find_map(|selector_text| {
        Selector::parse(selector_text)
            .ok()
            .and_then(|selector| first_attr(parent, &selector, attribute))
    })
}

fn first_poster_attr(
    parent: ElementRef<'_>,
    selector: &Selector,
    preferred_attribute: &str,
) -> Option<String> {
    let attributes = [
        preferred_attribute,
        "src",
        "data-src",
        "data-original",
        "data-lazy-src",
    ];
    parent.select(selector).next().and_then(|node| {
        attributes.iter().find_map(|attribute| {
            node.value()
                .attr(attribute)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
    })
}

fn first_poster_attr_any(
    parent: ElementRef<'_>,
    selectors: &[&str],
    preferred_attribute: &str,
) -> Option<String> {
    selectors.iter().find_map(|selector_text| {
        Selector::parse(selector_text)
            .ok()
            .and_then(|selector| first_poster_attr(parent, &selector, preferred_attribute))
    })
}

fn absolutize_url(base_url: &str, page_url: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("javascript:")
        || trimmed.starts_with("mailto:")
        || trimmed.starts_with('#')
    {
        return None;
    }
    if let Ok(parsed) = Url::parse(trimmed) {
        return Some(parsed.to_string());
    }
    Url::parse(page_url)
        .or_else(|_| Url::parse(base_url))
        .ok()
        .and_then(|base| base.join(trimmed).ok())
        .map(|url| url.to_string())
}

fn is_playable_video_url(value: &str) -> bool {
    let path = Url::parse(value)
        .ok()
        .map(|url| url.path().to_ascii_lowercase())
        .unwrap_or_else(|| value.split('?').next().unwrap_or(value).to_ascii_lowercase());
    [".mp4", ".m4v", ".webm", ".ogv", ".ogg", ".mov", ".m3u8", ".mpd"]
        .iter()
        .any(|extension| path.ends_with(extension))
}

struct CandidateScore {
    score: f64,
    reason: String,
}

fn score_result_candidate(query: &str, title: &str, candidate_year: Option<&str>) -> CandidateScore {
    let query_year = extract_year(query);
    let result_year = candidate_year.and_then(extract_year).or_else(|| extract_year(title));
    let query_title = normalize_comparable_title(&remove_years(query));
    let result_title = normalize_comparable_title(&remove_years(title));
    if query_title.is_empty() || result_title.is_empty() {
        return CandidateScore {
            score: 0.0,
            reason: "Missing title text.".to_string(),
        };
    }

    let mut score;
    let mut reason;
    if query_title == result_title && query_year.is_some() && result_year == query_year {
        score = 100.0;
        reason = "Exact title and year match.".to_string();
    } else if query_title == result_title {
        score = if query_year.is_some() { 82.0 } else { 90.0 };
        reason = if query_year.is_some() {
            "Exact title, but year is missing.".to_string()
        } else {
            "Exact title match.".to_string()
        };
    } else if result_title.contains(&query_title) {
        score = if query_year.is_some() && result_year == query_year {
            85.0
        } else {
            80.0
        };
        reason = if query_year.is_some() && result_year == query_year {
            "Title contains query and year matches.".to_string()
        } else {
            "Title contains query.".to_string()
        };
    } else if query_title.contains(&result_title) && result_title.len() >= 4 {
        score = 68.0;
        reason = "Query contains candidate title.".to_string();
    } else {
        score = word_overlap_score(&query_title, &result_title);
        let fuzzy = fuzzy_title_score(&query_title, &result_title);
        if fuzzy > score {
            score = fuzzy;
        }
        reason = if fuzzy >= 78.0 && fuzzy >= score {
            "High fuzzy title similarity.".to_string()
        } else if score >= 70.0 {
            "High word overlap.".to_string()
        } else {
            "Partial title overlap.".to_string()
        };
    }

    if query_year.is_some() && result_year == query_year {
        score = (score + 8.0_f64).min(100.0_f64);
        reason = format!("{reason} Year matches.");
    } else if query_year.is_some() && result_year.is_some() && result_year != query_year {
        score = (score - 35.0_f64).min(60.0_f64);
        reason = format!("{reason} Year differs from query.");
    }

    CandidateScore {
        score: score.max(0.0_f64).round(),
        reason,
    }
}

fn fuzzy_title_score(query_title: &str, result_title: &str) -> f64 {
    let score = (strsim::normalized_levenshtein(query_title, result_title) * 100.0).round();
    if score >= 55.0 {
        score
    } else {
        (score * 0.65).round()
    }
}

fn word_overlap_score(query_title: &str, result_title: &str) -> f64 {
    let query_tokens = content_tokens(query_title)
        .into_iter()
        .collect::<HashSet<_>>();
    let title_tokens = content_tokens(result_title)
        .into_iter()
        .collect::<HashSet<_>>();
    if query_tokens.is_empty() || title_tokens.is_empty() {
        return 0.0;
    }
    let overlap = query_tokens.intersection(&title_tokens).count() as f64;
    let query_coverage = overlap / query_tokens.len() as f64;
    let title_coverage = overlap / title_tokens.len() as f64;
    (query_coverage * 82.0).max(title_coverage * 68.0).round()
}

fn extract_year(value: &str) -> Option<String> {
    let mut digits = String::new();
    for character in value.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        if digits.len() == 4 && (digits.starts_with("19") || digits.starts_with("20")) {
            return Some(digits);
        }
        digits.clear();
    }
    None
}

fn remove_years(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|token| extract_year(token).is_none())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_match_key(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_alphanumeric() {
            for lower in character.to_lowercase() {
                normalized.push(lower);
            }
        } else {
            normalized.push(' ');
        }
    }
    collapse_whitespace(&normalized)
}

fn normalize_comparable_title(value: &str) -> String {
    let normalized = normalize_match_key(value);
    content_tokens(&normalized).join(" ")
}

fn content_tokens(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|token| token.len() > 1)
        .filter(|token| !is_noise_match_token(token))
        .map(str::to_string)
        .collect()
}

fn is_noise_match_token(token: &str) -> bool {
    matches!(
        token,
        "season" | "episode" | "series" | "show" | "movie" | "film" | "full" | "watch"
            | "online" | "hd" | "uhd"
            | "смотреть"
    ) || (token.len() <= 4
        && (token.starts_with('s') || token.starts_with('e'))
        && token.chars().skip(1).all(|character| character.is_ascii_digit()))
        || (token.len() <= 5
            && token.starts_with("ep")
            && token.chars().skip(2).all(|character| character.is_ascii_digit()))
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode_query_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

fn clean_string(value: Option<String>) -> Option<String> {
    value
        .map(|inner| inner.trim().to_string())
        .filter(|inner| !inner.is_empty())
}

fn normalized_source_kind(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("direct") {
        "direct".to_string()
    } else {
        "web".to_string()
    }
}

fn normalized_source_type(value: &str) -> String {
    match value.trim() {
        "directPage" => "directPage".to_string(),
        "webviewOnly" => "webviewOnly".to_string(),
        _ => "search".to_string(),
    }
}

fn normalized_source_open_behavior(value: &str) -> String {
    if value.trim() == "nativeThenWebview" {
        "nativeThenWebview".to_string()
    } else {
        "webview".to_string()
    }
}

fn normalized_result_open_behavior(value: &str) -> String {
    if value.trim() == "search_page" {
        "search_page".to_string()
    } else {
        "result_page".to_string()
    }
}

fn normalized_ambiguous_query_behavior(value: &str) -> String {
    if value.trim() == "open_search_page" {
        "open_search_page".to_string()
    } else {
        "show_choices".to_string()
    }
}

fn normalized_parser_mode(value: &str, source_type: &str, requires_javascript: bool) -> String {
    match value.trim() {
        "static" => "static".to_string(),
        "webview" => "webview".to_string(),
        "hybrid" => "hybrid".to_string(),
        "fallbackOnly" => "fallbackOnly".to_string(),
        _ if source_type.trim() == "webviewOnly" => "fallbackOnly".to_string(),
        _ if requires_javascript => "hybrid".to_string(),
        _ => "static".to_string(),
    }
}

fn normalize_watch_patterns(patterns: Vec<String>) -> Vec<String> {
    let normalized = patterns
        .into_iter()
        .map(|pattern| pattern.trim().to_string())
        .filter(|pattern| !pattern.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        default_watch_link_text_patterns()
    } else {
        normalized
    }
}

fn is_web_source(source: &SourceConfig) -> bool {
    !source.source_kind.trim().eq_ignore_ascii_case("direct")
}

fn open_url_with_system(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let status = Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .status();

    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(url).status();

    #[cfg(all(unix, not(target_os = "macos")))]
    let status = Command::new("xdg-open").arg(url).status();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("System browser exited with status {status}.")),
        Err(error) => Err(format!("Could not open system browser: {error}")),
    }
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn stable_id(value: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("item-{hash:x}")
}

fn sample_sources() -> Vec<SourceConfig> {
    vec![
        create_default_source(
            "roku-channel",
            "The Roku Channel",
            "https://therokuchannel.roku.com",
            "https://therokuchannel.roku.com/search/{query}",
            "Free ad-supported catalog; region and account behavior can vary.",
        ),
        create_default_source(
            "sling-freestream",
            "Sling Freestream",
            "https://watch.sling.com",
            "https://watch.sling.com/search?query={query}",
            "Free streaming area; region and WebView behavior can vary.",
        ),
        create_default_source(
            "fawesome",
            "Fawesome",
            "https://fawesome.tv",
            "https://fawesome.tv/search?query={query}",
            "Free ad-supported catalog; WebView fallback may be needed.",
        ),
        create_default_source(
            "youtube-movies-tv",
            "YouTube Movies & TV",
            "https://www.youtube.com/feed/storefront",
            "https://www.youtube.com/results?search_query={query}",
            "May include rentals, purchases, clips, and region-dependent free titles.",
        ),
        create_default_source(
            "nfb-ca",
            "NFB.ca",
            "https://www.nfb.ca",
            "https://www.nfb.ca/search/?q={query}",
            "National Film Board of Canada catalog; some items may be region-limited.",
        ),
    ]
}

fn create_default_source(
    default_source_id: &str,
    name: &str,
    base_url: &str,
    search_url: &str,
    note: &str,
) -> SourceConfig {
    SourceConfig {
        id: format!("default-{default_source_id}"),
        name: name.to_string(),
        enabled: true,
        default_source_id: Some(default_source_id.to_string()),
        is_default: true,
        user_modified: false,
        hidden: false,
        is_deleted: false,
        deleted_at: None,
        note: Some(note.to_string()),
        source_kind: "web".to_string(),
        source_type: "search".to_string(),
        source_open_behavior: "webview".to_string(),
        result_open_behavior: "result_page".to_string(),
        ambiguous_query_behavior: "show_choices".to_string(),
        parser_mode: "hybrid".to_string(),
        base_url: base_url.to_string(),
        search_url: search_url.to_string(),
        method: "GET".to_string(),
        result_selector: String::new(),
        load_delay_ms: 1500,
        max_retries: 2,
        request_timeout_ms: 15000,
        wait_for_selector: None,
        title_selector: None,
        poster_selector: None,
        poster_attribute: Some("src".to_string()),
        link_selector: None,
        link_attribute: Some("href".to_string()),
        year_selector: None,
        description_selector: None,
        video_selector: None,
        video_attribute: Some("src".to_string()),
        iframe_selector: Some("iframe".to_string()),
        iframe_attribute: Some("src".to_string()),
        subtitle_selector: None,
        subtitle_attribute: Some("src".to_string()),
        subtitle_language_attribute: Some("srclang".to_string()),
        audio_language_selector: None,
        download_selector: None,
        download_attribute: Some("href".to_string()),
        watch_button_selector: None,
        watch_link_text_patterns: default_watch_link_text_patterns(),
        episode_selector: None,
        season_selector: None,
        player_selector: Some("video, iframe".to_string()),
        auto_resolve_watch_page: true,
        auto_open_first_watch_link: false,
        auto_open_best_match: true,
        auto_open_watch_button: true,
        max_watch_resolve_steps: 2,
        max_resolve_steps: 2,
        resolve_delay_ms: 1500,
        exact_match_threshold: 85,
        requires_javascript: true,
        headers: HashMap::new(),
        created_at: None,
        updated_at: None,
    }
}

fn default_source_by_id(default_source_id: &str) -> Option<SourceConfig> {
    sample_sources()
        .into_iter()
        .find(|source| source.default_source_id.as_deref() == Some(default_source_id))
}

fn infer_default_metadata(source: SourceConfig) -> SourceConfig {
    if source.default_source_id.is_some() {
        return SourceConfig {
            is_default: true,
            ..source
        };
    }

    let normalized_name = source.name.to_lowercase();
    for default_source in sample_sources() {
        if default_source.name.to_lowercase() == normalized_name {
            return SourceConfig {
                default_source_id: default_source.default_source_id,
                is_default: true,
                ..source
            };
        }
    }

    source
}

fn is_removed_or_wrong_source(source: &SourceConfig) -> bool {
    let normalized_name = source.name.to_lowercase();
    let normalized_base_url = source.base_url.to_lowercase();
    if normalized_name.contains("prada")
        || normalized_base_url.contains("prada")
        || (normalized_name.contains("example movie source")
            && normalized_base_url.contains("example.com"))
    {
        return true;
    }

    if let Some(default_source_id) = source.default_source_id.as_deref() {
        if removed_default_source_ids().contains(default_source_id) {
            return true;
        }
    }

    let removed_names = ["plex", "tubi", "pluto", "filmzie", "xumo", "filmrise", "arte"];
    (source.is_default || source.default_source_id.is_some())
        && removed_names
            .iter()
            .any(|name| normalized_name.contains(name))
}

fn removed_default_source_ids() -> HashSet<&'static str> {
    HashSet::from([
        "plex",
        "tubi",
        "pluto-tv",
        "filmzie",
        "xumo-play",
        "filmrise",
        "arte-tv",
    ])
}

fn favorite_from_row(row: &Row<'_>) -> rusqlite::Result<Favorite> {
    Ok(Favorite {
        id: row.get(0)?,
        title: row.get(1)?,
        source_name: row.get(2)?,
        url: row.get(3)?,
        open_mode: row.get(4)?,
        playable_url: row.get(5)?,
        poster_url: row.get(6)?,
        created_at: Some(row.get(7)?),
    })
}

fn history_from_row(row: &Row<'_>) -> rusqlite::Result<HistoryItem> {
    Ok(HistoryItem {
        id: row.get(0)?,
        title: row.get(1)?,
        source_name: row.get(2)?,
        url: row.get(3)?,
        open_mode: row.get(4)?,
        playable_url: row.get(5)?,
        poster_url: row.get(6)?,
        last_opened_at: Some(row.get(7)?),
        playback_position_seconds: row.get(8)?,
        duration_seconds: row.get(9)?,
    })
}

fn collect_rows<T>(
    rows: impl Iterator<Item = rusqlite::Result<T>>,
    label: &str,
) -> Result<Vec<T>, String> {
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("Could not collect {label}: {error}"))
}
