#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Utc;
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
use tauri::{Manager, State};
use url::Url;

const DEFAULT_USER_AGENT: &str = "CineFinder/0.1 local desktop aggregator";

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
    note: Option<String>,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    #[serde(default = "default_source_type")]
    source_type: String,
    #[serde(default = "default_source_open_behavior")]
    source_open_behavior: String,
    #[serde(default = "default_result_open_behavior")]
    result_open_behavior: String,
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
    #[serde(default)]
    episode_selector: Option<String>,
    #[serde(default)]
    season_selector: Option<String>,
    #[serde(default)]
    player_selector: Option<String>,
    #[serde(default)]
    auto_open_first_watch_link: bool,
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
    detected_selectors: Vec<SelectorCandidate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourcePreviewResult {
    title: String,
    url: String,
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
            open_external_url
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
    validate_source(&source)?;
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    save_source_to_db(&conn, source)
}

#[tauri::command]
fn delete_source(state: State<'_, AppState>, source_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let target = read_source_by_id(&conn, &source_id)?;
    if target.is_default || target.default_source_id.is_some() {
        save_source_to_db(
            &conn,
            SourceConfig {
                enabled: false,
                hidden: true,
                user_modified: true,
                ..target
            },
        )?;
    } else {
        conn.execute("DELETE FROM sources WHERE id = ?1", params![source_id])
            .map_err(|error| format!("Could not delete source: {error}"))?;
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
        .filter(|source| {
            selected_ids
                .as_ref()
                .map(|ids| ids.contains(&source.id))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    let tasks = searchable_sources
        .into_iter()
        .map(|source| search_single_source(source, trimmed_query.clone()));
    Ok(join_all(tasks).await)
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
        note: clean_string(source.note),
        source_kind: normalized_source_kind(&source.source_kind),
        source_type: normalized_source_type(&source.source_type),
        source_open_behavior: normalized_source_open_behavior(&source.source_open_behavior),
        result_open_behavior: normalized_result_open_behavior(&source.result_open_behavior),
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
        episode_selector: clean_string(source.episode_selector),
        season_selector: clean_string(source.season_selector),
        player_selector: clean_string(source.player_selector).or_else(|| Some("video, iframe".to_string())),
        auto_open_first_watch_link: source.auto_open_first_watch_link,
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
    let sources = read_sources(conn)?;
    let mut seen_default_ids = HashSet::new();

    for source in sources {
        let source = infer_default_metadata(source);
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
                            created_at: source.created_at,
                            updated_at: source.updated_at,
                            ..default_source
                        },
                    )?;
                } else if restore_hidden && source.hidden {
                    save_source_to_db(
                        conn,
                        SourceConfig {
                            hidden: false,
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
        save_source_to_db(conn, default_source)?;
    }

    Ok(())
}

async fn search_single_source(source: SourceConfig, query: String) -> SourceSearchOutcome {
    let started = Instant::now();

    if source.source_type == "webviewOnly" {
        let url = build_search_url(&source, &query);
        return outcome(
            &source,
            "ready",
            Some("WebView-only source. Open source search page.".to_string()),
            started,
            vec![webview_result(&source, &query, &url)],
        );
    }
    if source.result_open_behavior == "search_page" {
        let url = build_search_url(&source, &query);
        return outcome(
            &source,
            "ready",
            Some("Configured to open the source search page.".to_string()),
            started,
            vec![webview_result(&source, &query, &url)],
        );
    }

    if let Err(error) = validate_source(&source) {
        return outcome(&source, "error", Some(error), started, Vec::new());
    }

    if source.method.to_uppercase() != "GET" {
        return outcome(
            &source,
            "error",
            Some("Only GET source searches are supported in v1.".to_string()),
            started,
            Vec::new(),
        );
    }

    let search_url = build_search_url(&source, &query);
    if source.source_type == "directPage" {
        return outcome(
            &source,
            "found",
            Some("Direct page source. Opening configured page.".to_string()),
            started,
            vec![direct_page_result(&source, &query, &search_url)],
        );
    }

    let headers = match build_headers(&source.headers) {
        Ok(headers) => headers,
        Err(error) => return outcome(&source, "error", Some(error), started, Vec::new()),
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(source_timeout_ms(&source)))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return outcome(
                &source,
                "error",
                Some(format!("Could not create HTTP client: {error}")),
                started,
                Vec::new(),
            )
        }
    };

    let html = match fetch_search_html_with_retries(&client, &headers, &source, &search_url).await {
        Ok(html) => html,
        Err(FetchFailure::TimedOut(message)) => {
            return outcome(&source, "timed_out", Some(message), started, Vec::new())
        }
        Err(FetchFailure::SelectorMissing(message)) => {
            return outcome(
                &source,
                "ready",
                Some(format!("{message} Open source search page.")),
                started,
                vec![webview_result(&source, &query, &search_url)],
            )
        }
        Err(FetchFailure::Failed(message)) => {
            return outcome(&source, "error", Some(message), started, Vec::new())
        }
    };

    match parse_results(&source, &query, &search_url, &html) {
        Ok(candidates) => {
            let results =
                resolve_playable_results(&client, &headers, &source, &search_url, candidates)
                    .await;
            if results.is_empty() {
                outcome(
                    &source,
                    "ready",
                    Some("No parsed results. Open source search page.".to_string()),
                    started,
                    vec![webview_result(&source, &query, &search_url)],
                )
            } else {
                outcome(
                    &source,
                    "found",
                    Some(format!(
                        "Parsed {} result(s). Best match opens the exact result page.",
                        results.len()
                    )),
                    started,
                    results,
                )
            }
        }
        Err(error) => outcome(&source, "error", Some(error), started, Vec::new()),
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
            }],
            fallback_used: true,
            best_match: None,
            final_open_url: Some(build_search_url(&source, &query)),
            detected_selectors: Vec::new(),
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
            }],
            fallback_used: true,
            best_match: None,
            final_open_url: Some(final_search_url),
            detected_selectors: Vec::new(),
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
            }],
            fallback_used: false,
            best_match: Some(SourcePreviewResult {
                title: source.name.clone(),
                url: final_search_url.clone(),
            }),
            final_open_url: Some(final_search_url),
            detected_selectors: Vec::new(),
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
                detected_selectors: Vec::new(),
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
                detected_selectors: Vec::new(),
            }
        }
    };

    match fetch_search_html_with_retries(&client, &headers, &source, &final_search_url).await {
        Ok(html) => {
            let selector_match_count = selector_match_count(&source, &html).unwrap_or(0);
            let detected_selectors = detect_selector_candidates(&html);
            let parsed_results = parse_results(&source, &query, &final_search_url, &html)
                .unwrap_or_default();
            let best_match = parsed_results.first().map(|result| SourcePreviewResult {
                title: result.title.clone(),
                url: result.url.clone(),
            });
            let preview_results = parsed_results
                .into_iter()
                .take(5)
                .map(|result| SourcePreviewResult {
                    title: result.title,
                    url: result.url,
                })
                .collect::<Vec<_>>();

            SourceTestResult {
                ok: true,
                message: if best_match.is_some() {
                    "Source loaded and parsed exact result pages.".to_string()
                } else {
                    "Source loaded, but no matching result cards were parsed.".to_string()
                },
                result_count: preview_results.len(),
                elapsed_ms: started.elapsed().as_millis(),
                final_search_url: Some(final_search_url.clone()),
                raw_status: Some("loaded".to_string()),
                selector_match_count,
                preview_results,
                fallback_used: best_match.is_none(),
                final_open_url: best_match
                    .as_ref()
                    .map(|result| result.url.clone())
                    .or_else(|| Some(final_search_url)),
                best_match,
                detected_selectors,
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
            detected_selectors: Vec::new(),
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
            detected_selectors: Vec::new(),
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
            detected_selectors: Vec::new(),
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
            tauri::async_runtime::sleep(Duration::from_millis(700 + u64::from(attempt) * 400))
                .await;
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
        tauri::async_runtime::sleep(Duration::from_millis(delay_ms)).await;
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
    if source.watch_button_selector.is_none() && !source.auto_open_first_watch_link {
        return None;
    }

    let response = client
        .get(result_url)
        .headers(headers.clone())
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let html = response.text().await.ok()?;
    let document = Html::parse_document(&html);
    let selector_values = source
        .watch_button_selector
        .as_deref()
        .map(|selector| vec![selector.to_string()])
        .unwrap_or_else(|| {
            vec![
                "a[href*='watch']".to_string(),
                "a[href*='play']".to_string(),
                "a[href*='episode']".to_string(),
                "[data-href]".to_string(),
                "[data-url]".to_string(),
            ]
        });

    for selector_text in selector_values {
        let Ok(selector) = Selector::parse(selector_text.trim()) else {
            continue;
        };
        for node in document.select(&selector) {
            let raw_url = node
                .value()
                .attr("href")
                .or_else(|| node.value().attr("data-href"))
                .or_else(|| node.value().attr("data-url"));
            if let Some(url) = raw_url.and_then(|value| absolutize_url(&source.base_url, result_url, value)) {
                return Some(url);
            }
        }
    }

    None
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
            .filter(|value| !value.is_empty());
        let description = description_selector
            .as_ref()
            .and_then(|selector| first_text(node, selector))
            .filter(|value| !value.is_empty());
        let poster_url = poster_selector
            .as_ref()
            .and_then(|selector| first_attr(node, selector, poster_attribute))
            .or_else(|| first_attr_any(node, common_poster_selectors(), poster_attribute))
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

        let confidence = confidence_score(query, &title);
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
            raw_data,
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

fn outcome(
    source: &SourceConfig,
    status: &str,
    message: Option<String>,
    started: Instant,
    results: Vec<SearchResult>,
) -> SourceSearchOutcome {
    SourceSearchOutcome {
        source_id: source.id.clone(),
        source_name: source.name.clone(),
        status: status.to_string(),
        message,
        elapsed_ms: started.elapsed().as_millis(),
        results,
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
    template
        .replace("{query}", &encoded_query)
        .replace("{slug}", &slug_query)
}

fn source_timeout_ms(source: &SourceConfig) -> u64 {
    source.request_timeout_ms.clamp(3_000, 60_000)
}

fn webview_result(source: &SourceConfig, query: &str, url: &str) -> SearchResult {
    let mut raw_data = HashMap::new();
    raw_data.insert("provider".to_string(), "javascript-webview".to_string());
    raw_data.insert("resultKind".to_string(), "provider".to_string());
    raw_data.insert("primary".to_string(), "true".to_string());

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
        confidence: 100.0,
        raw_data,
    }
}

fn direct_page_result(source: &SourceConfig, query: &str, url: &str) -> SearchResult {
    let mut raw_data = HashMap::new();
    raw_data.insert("provider".to_string(), "direct-page".to_string());
    raw_data.insert("resultKind".to_string(), "parsed".to_string());
    raw_data.insert("primary".to_string(), "true".to_string());
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
        raw_data,
    }
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
        "[title]",
    ]
}

fn common_link_selectors() -> &'static [&'static str] {
    &["a[href]", ".title a", ".movie-title a", ".poster a", ".thumb a"]
}

fn common_poster_selectors() -> &'static [&'static str] {
    &["img", ".poster img", ".thumb img", "picture img"]
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

fn confidence_score(query: &str, title: &str) -> f64 {
    let normalized_query = normalize_match_key(query);
    let normalized_title = normalize_match_key(title);
    if normalized_query.is_empty() || normalized_title.is_empty() {
        return 0.0;
    }
    if normalized_title == normalized_query {
        return 100.0;
    }
    if normalized_title.contains(&normalized_query) {
        return 92.0;
    }
    if normalized_query.contains(&normalized_title) && normalized_title.len() >= 4 {
        return 74.0;
    }

    let query_tokens = content_tokens(&normalized_query)
        .into_iter()
        .collect::<HashSet<_>>();
    let title_tokens = content_tokens(&normalized_title)
        .into_iter()
        .collect::<HashSet<_>>();
    if query_tokens.is_empty() {
        return 0.0;
    }

    let overlap = query_tokens.intersection(&title_tokens).count() as f64;
    let query_coverage = overlap / query_tokens.len() as f64;
    let title_coverage = if title_tokens.is_empty() {
        0.0
    } else {
        overlap / title_tokens.len() as f64
    };
    (query_coverage * 86.0).max(title_coverage * 70.0).round()
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
        note: Some(note.to_string()),
        source_kind: "web".to_string(),
        source_type: "search".to_string(),
        source_open_behavior: "webview".to_string(),
        result_open_behavior: "result_page".to_string(),
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
        episode_selector: None,
        season_selector: None,
        player_selector: Some("video, iframe".to_string()),
        auto_open_first_watch_link: false,
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
