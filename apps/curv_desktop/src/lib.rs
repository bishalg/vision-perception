//! Curv Console desktop shell (WP-D.9, ADR-0011 decision 10): a Tauri 2 window that loads
//! the clinic server's console URL ("remote mode"). The console is not bundled.
//!
//! Two windows, never mixed:
//! - `setup`: the bundled static page (`setup/`), the only webview with an IPC capability
//!   (`set_server_url`), allowed to navigate only within the local app origin.
//! - `main`: the remote console, no capability at all, navigation pinned to the one
//!   configured server origin; other links open in the system browser.

pub mod policy;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{AppHandle, Manager, Runtime, Webview, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use policy::{classify_navigation, is_local_app_url, Navigation};

const MAIN_WINDOW: &str = "main";
const SETUP_WINDOW: &str = "setup";
const CONFIG_FILE: &str = "server.json";
const MENU_CHANGE_SERVER: &str = "change_server";
const MENU_RELOAD: &str = "reload";
const APP_TITLE: &str = "Curv Console";

/// The configured server URL, read by the navigation handlers on every request.
#[derive(Default, Clone)]
struct ServerState(Arc<RwLock<Option<Url>>>);

impl ServerState {
    fn get(&self) -> Option<Url> {
        self.0.read().ok().and_then(|g| g.clone())
    }
    fn set(&self, url: Url) {
        if let Ok(mut g) = self.0.write() {
            *g = Some(url);
        }
    }
}

fn config_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join(CONFIG_FILE))
}

fn load_config<R: Runtime>(app: &AppHandle<R>) -> Option<Url> {
    let raw = std::fs::read_to_string(config_path(app)?).ok()?;
    policy::parse_config(&raw)
}

fn save_config<R: Runtime>(app: &AppHandle<R>, url: &Url) -> Result<(), String> {
    let path = config_path(app).ok_or("No app config directory on this system.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create config dir: {e}"))?;
    }
    std::fs::write(&path, policy::serialize_config(url))
        .map_err(|e| format!("Cannot save config: {e}"))
}

/// Opens an off-origin link in the system browser (http, https, mailto only; the policy
/// never returns `OpenExternal` for anything else).
fn open_external<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        eprintln!("curv-desktop: could not open external link: {e}");
    }
}

/// Shows the console for `server`: navigates the existing console window or creates it.
fn open_console<R: Runtime>(app: &AppHandle<R>, server: Url) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        win.navigate(server)?;
        win.set_focus()?;
        return Ok(());
    }

    let state = app.state::<ServerState>().inner().clone();
    let nav_app = app.clone();
    let nav_state = state.clone();
    let new_win_app = app.clone();
    let new_win_state = state.clone();
    let dl_state = state;

    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::External(server))
        .title(APP_TITLE)
        .inner_size(1280.0, 860.0)
        .min_inner_size(900.0, 600.0)
        .devtools(cfg!(debug_assertions))
        .on_navigation(move |target| {
            let Some(server) = nav_state.get() else {
                return false;
            };
            match classify_navigation(&server, target) {
                Navigation::Allow => true,
                Navigation::OpenExternal => {
                    open_external(&nav_app, target);
                    false
                }
                Navigation::Block => false,
            }
        })
        // window.open / target=_blank: never a second webview. Same-origin targets load
        // in the console window (keeps the session), others go to the system browser.
        .on_new_window(move |target, _features| {
            if let Some(server) = new_win_state.get() {
                match classify_navigation(&server, &target) {
                    Navigation::Allow => {
                        if let Some(win) = new_win_app.get_webview_window(MAIN_WINDOW) {
                            let _ = win.navigate(target);
                        }
                    }
                    Navigation::OpenExternal => open_external(&new_win_app, &target),
                    Navigation::Block => {}
                }
            }
            NewWindowResponse::Deny
        })
        // Downloads (case ZIP, CSV, PDF) only from the server origin, to the OS default
        // download location.
        .on_download(move |_webview, event| match event {
            DownloadEvent::Requested { url, .. } => dl_state
                .get()
                .is_some_and(|s| classify_navigation(&s, &url) == Navigation::Allow),
            _ => true,
        })
        .build()?;
    Ok(())
}

/// Shows the bundled setup page (first run, or the "Change clinic server" menu item).
fn open_setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window(SETUP_WINDOW) {
        win.set_focus()?;
        return Ok(());
    }
    let current = app
        .state::<ServerState>()
        .get()
        .map(|u| u.to_string())
        .unwrap_or_default();
    // JSON-encoded, so the value is a string literal whatever it contains. Injected by
    // the webview (not subject to the page CSP); the page only prefills the input.
    let prefill = format!(
        "window.__CURV_CURRENT_SERVER__ = {};",
        serde_json::to_string(&current).unwrap_or_else(|_| "\"\"".into())
    );

    WebviewWindowBuilder::new(app, SETUP_WINDOW, WebviewUrl::App("index.html".into()))
        .title(format!("{APP_TITLE}: clinic server"))
        .inner_size(560.0, 420.0)
        .resizable(false)
        .devtools(cfg!(debug_assertions))
        .initialization_script(prefill)
        .on_navigation(is_local_app_url)
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .build()?;
    Ok(())
}

/// The one IPC command. Callable only from the `setup` window (ACL capability
/// `capabilities/setup.json`); the label and origin checks below are defence in depth.
#[tauri::command]
fn set_server_url<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    url: String,
) -> Result<String, String> {
    let caller_is_setup = webview.label() == SETUP_WINDOW
        && webview.url().map(|u| is_local_app_url(&u)).unwrap_or(false);
    if !caller_is_setup {
        return Err("Not allowed.".into());
    }
    let server = policy::validate_server_url(&url).map_err(|e| e.to_string())?;
    save_config(&app, &server)?;
    app.state::<ServerState>().set(server.clone());
    open_console(&app, server.clone()).map_err(|e| format!("Cannot open the console: {e}"))?;
    if let Some(win) = app.get_webview_window(SETUP_WINDOW) {
        let _ = win.close();
    }
    Ok(server.to_string())
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let change_server = MenuItem::with_id(
        app,
        MENU_CHANGE_SERVER,
        "Change Clinic Server…",
        true,
        None::<&str>,
    )?;
    let reload = MenuItem::with_id(app, MENU_RELOAD, "Reload", true, Some("CmdOrCtrl+R"))?;

    let app_menu = Submenu::with_items(
        app,
        APP_TITLE,
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &change_server,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[&reload, &PredefinedMenuItem::fullscreen(app, None)?],
    )?;
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&app_menu, &edit_menu, &view_menu, &window_menu])
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ServerState::default())
        .invoke_handler(tauri::generate_handler![set_server_url])
        .menu(build_menu)
        .on_menu_event(|app, event| {
            let result = match event.id().as_ref() {
                MENU_CHANGE_SERVER => open_setup(app),
                MENU_RELOAD => match app.get_webview_window(MAIN_WINDOW) {
                    Some(win) => win.reload(),
                    None => Ok(()),
                },
                _ => Ok(()),
            };
            if let Err(e) = result {
                eprintln!("curv-desktop: menu action failed: {e}");
            }
        })
        .setup(|app| {
            let handle = app.handle();
            match load_config(handle) {
                Some(server) => {
                    app.state::<ServerState>().set(server.clone());
                    open_console(handle, server)?;
                }
                None => open_setup(handle)?,
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Curv Console desktop shell");
}
