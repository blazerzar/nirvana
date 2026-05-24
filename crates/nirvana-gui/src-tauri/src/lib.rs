use nirvana_core::api::domain::{
    AppSettings, Change, Connection, ConnectionData, PublishFailure, PublishResult, Slot,
    SlotCreate, SlotEdit, Ticket,
};
use nirvana_core::api::{NirvanaApi, SlotSort};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main";
const TRAY_STATUS_ID: &str = "status";
const TRAY_SHOW_ID: &str = "show";
const TRAY_QUIT_ID: &str = "quit";

struct TrayStatusState {
    item: MenuItem<tauri::Wry>,
}

#[derive(Serialize)]
struct GuiConnection {
    id: i64,
    name: String,
    #[serde(rename = "type")]
    connection_type: String,
    hostname: String,
    username: String,
}

#[derive(Deserialize)]
struct CreateConnectionInput {
    name: String,
    #[serde(rename = "type")]
    connection_type: String,
    hostname: String,
    username: String,
    token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionIdInput {
    connection_id: i64,
}

#[derive(Serialize)]
struct GuiSlot {
    id: i64,
    ticket_key: String,
    summary: Option<String>,
    note: Option<String>,
    started_at: i64,
    stopped_at: Option<i64>,
    published_at: Option<i64>,
    issue_url: Option<String>,
}

#[derive(Serialize)]
struct GuiTicket {
    id: i64,
    ticket_key: String,
    summary: Option<String>,
    issue_url: Option<String>,
}

#[derive(Deserialize)]
struct ListSlotsInput {
    from: i64,
    to: Option<i64>,
    sort: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartSlotInput {
    ticket_key: String,
    note: Option<String>,
    started_at: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSlotInput {
    ticket_key: String,
    note: Option<String>,
    started_at: i64,
    stopped_at: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditSlotInput {
    slot_id: i64,
    note: String,
    started_at: i64,
    stopped_at: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSlotInput {
    slot_id: i64,
}

#[derive(Deserialize)]
struct PublishSlotsInput {
    from: i64,
    to: Option<i64>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiSettings {
    publish_squashed_worklogs: bool,
    font_scale: f64,
    theme: String,
    show_tray_icon: bool,
}

#[derive(Serialize)]
struct GuiPublishFailure {
    ticket_key: String,
    error: String,
}

#[derive(Serialize)]
struct GuiPublishResult {
    published_count: usize,
    failed: Vec<GuiPublishFailure>,
    timestamp: i64,
}

impl From<Connection> for GuiConnection {
    fn from(connection: Connection) -> Self {
        Self {
            id: connection.id,
            name: connection.name,
            connection_type: connection.kind,
            hostname: connection.host,
            username: connection.identity,
        }
    }
}

impl From<Slot> for GuiSlot {
    fn from(slot: Slot) -> Self {
        Self {
            id: slot.id,
            ticket_key: slot.ticket_key,
            summary: slot.summary,
            note: slot.note,
            started_at: slot.started_at,
            stopped_at: slot.stopped_at,
            published_at: slot.published_at,
            issue_url: slot.issue_url,
        }
    }
}

impl From<Ticket> for GuiTicket {
    fn from(ticket: Ticket) -> Self {
        Self {
            id: ticket.id,
            ticket_key: ticket.ticket_key.clone(),
            summary: ticket.summary,
            issue_url: None,
        }
    }
}

impl From<PublishFailure> for GuiPublishFailure {
    fn from(failure: PublishFailure) -> Self {
        Self {
            ticket_key: failure.ticket_key,
            error: failure.error,
        }
    }
}

impl From<PublishResult> for GuiPublishResult {
    fn from(result: PublishResult) -> Self {
        Self {
            published_count: result.published_count,
            failed: result
                .failed
                .into_iter()
                .map(GuiPublishFailure::from)
                .collect(),
            timestamp: result.timestamp,
        }
    }
}

impl From<AppSettings> for GuiSettings {
    fn from(settings: AppSettings) -> Self {
        Self {
            publish_squashed_worklogs: settings.publish_squashed_worklogs,
            font_scale: settings.font_scale,
            theme: settings.theme,
            show_tray_icon: settings.show_tray_icon,
        }
    }
}

impl From<GuiSettings> for AppSettings {
    fn from(settings: GuiSettings) -> Self {
        Self {
            publish_squashed_worklogs: settings.publish_squashed_worklogs,
            font_scale: settings.font_scale,
            theme: settings.theme,
            show_tray_icon: settings.show_tray_icon,
        }
    }
}

#[tauri::command]
fn get_app_info(app: tauri::AppHandle) -> serde_json::Value {
    let info = app.package_info();

    serde_json::json!({
        "name": info.name,
        "version": info.version.to_string(),
        "authors": info.authors,
        "description": info.description,
    })
}

#[tauri::command]
fn get_settings() -> Result<GuiSettings, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    Ok(api.get_settings().into())
}

#[tauri::command]
fn update_settings(input: GuiSettings) -> Result<GuiSettings, String> {
    let mut api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.update_settings(input.into())
        .map(GuiSettings::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_active_connection() -> Result<Option<GuiConnection>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.get_active_connection()
        .map(|connection| connection.map(GuiConnection::from))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_connections() -> Result<Vec<GuiConnection>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.list_connections()
        .map(|connections| connections.into_iter().map(GuiConnection::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_active_connection(
    app: tauri::AppHandle,
    input: ConnectionIdInput,
) -> Result<Option<GuiConnection>, String> {
    let mut api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.set_active_connection(input.connection_id)
        .map_err(|error| error.to_string())?;

    let connection = api
        .get_active_connection()
        .map(|connection| connection.map(GuiConnection::from))
        .map_err(|error| error.to_string())?;
    refresh_tray_status(&app);
    Ok(connection)
}

#[tauri::command]
fn delete_connection(
    app: tauri::AppHandle,
    input: ConnectionIdInput,
) -> Result<Option<GuiConnection>, String> {
    let mut api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.delete_connection(input.connection_id)
        .map_err(|error| error.to_string())?;

    let connection = api
        .get_active_connection()
        .map(|connection| connection.map(GuiConnection::from))
        .map_err(|error| error.to_string())?;
    refresh_tray_status(&app);
    Ok(connection)
}

#[tauri::command]
fn create_connection(
    app: tauri::AppHandle,
    input: CreateConnectionInput,
) -> Result<GuiConnection, String> {
    let mut api = NirvanaApi::new().map_err(|error| error.to_string())?;
    let data = ConnectionData {
        name: input.name,
        kind: input.connection_type,
        host: input.hostname,
        identity: input.username,
        secret_store: "plaintext".to_string(),
        token: input.token,
    };

    api.test_connection_data(ConnectionData {
        name: data.name.clone(),
        kind: data.kind.clone(),
        host: data.host.clone(),
        identity: data.identity.clone(),
        secret_store: data.secret_store.clone(),
        token: data.token.clone(),
    })
    .map_err(|error| error.to_string())?;

    let connection = api
        .add_connection(data)
        .map_err(|error| error.to_string())?;
    api.set_active_connection(connection.id)
        .map_err(|error| error.to_string())?;

    refresh_tray_status(&app);
    Ok(connection.into())
}

#[tauri::command]
fn list_slots(input: ListSlotsInput) -> Result<Vec<GuiSlot>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    let sort = match input.sort.as_deref() {
        Some("ticket") => SlotSort::TicketId,
        _ => SlotSort::StartedAt,
    };

    api.get_slots(input.from, input.to, sort)
        .map(|slots| slots.into_iter().map(GuiSlot::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_unpublished_slots(input: PublishSlotsInput) -> Result<Vec<GuiSlot>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.get_unpublished_slots(input.from, input.to)
        .map(|slots| slots.into_iter().map(GuiSlot::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_recent_tickets() -> Result<Vec<GuiTicket>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.list_recent_tickets()
        .map(|tickets| tickets.into_iter().map(GuiTicket::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_running_slot() -> Result<Option<GuiSlot>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.get_running_slot()
        .map(|slot| slot.map(GuiSlot::from))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_slot(app: tauri::AppHandle, input: StartSlotInput) -> Result<GuiSlot, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    let slot = api
        .start_slot(&input.ticket_key, input.started_at, input.note.as_deref())
        .map(GuiSlot::from)
        .map_err(|error| error.to_string())?;
    refresh_tray_status(&app);
    Ok(slot)
}

#[tauri::command]
fn create_slot(input: CreateSlotInput) -> Result<GuiSlot, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.create_slot(SlotCreate {
        ticket_key: input.ticket_key,
        note: input.note,
        started_at: input.started_at,
        stopped_at: input.stopped_at,
    })
    .map(GuiSlot::from)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn edit_slot(input: EditSlotInput) -> Result<GuiSlot, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    let note = input.note.trim();
    let edit = SlotEdit {
        note: if note.is_empty() {
            Change::Clear
        } else {
            Change::Set(note.to_string())
        },
        started_at: Some(input.started_at),
        stopped_at: input.stopped_at.map_or(Change::Clear, Change::Set),
    };

    api.edit_slot(input.slot_id, edit)
        .map(GuiSlot::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_slot(app: tauri::AppHandle, input: DeleteSlotInput) -> Result<GuiSlot, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    let slot = api
        .delete_slot(input.slot_id)
        .map(GuiSlot::from)
        .map_err(|error| error.to_string())?;
    refresh_tray_status(&app);
    Ok(slot)
}

#[tauri::command]
fn stop_slot(app: tauri::AppHandle) -> Result<Option<GuiSlot>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    let slot = api
        .stop_slot(None)
        .map(|slot| slot.map(GuiSlot::from))
        .map_err(|error| error.to_string())?;
    refresh_tray_status(&app);
    Ok(slot)
}

#[tauri::command]
fn publish_slots(input: PublishSlotsInput) -> Result<GuiPublishResult, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.publish(input.from, input.to)
        .map(GuiPublishResult::from)
        .map_err(|error| error.to_string())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.unminimize() {
            eprintln!("failed to unminimize main window: {error}");
        }

        if let Err(error) = window.show() {
            eprintln!("failed to show main window: {error}");
        }

        if let Err(error) = window.set_focus() {
            eprintln!("failed to focus main window: {error}");
        }
    }
}

fn refresh_tray_status(app: &tauri::AppHandle) {
    let running_slot = NirvanaApi::new()
        .and_then(|api| api.get_running_slot())
        .map_err(|error| {
            eprintln!("failed to load running slot for tray status: {error}");
            error
        })
        .ok()
        .flatten();

    let title = running_slot.as_ref().map(|slot| slot.ticket_key.as_str());
    let tooltip = running_slot
        .as_ref()
        .map(|slot| format!("nirvana · {}", slot.ticket_key))
        .unwrap_or_else(|| "nirvana".to_string());
    let menu_status = running_slot
        .as_ref()
        .map(|slot| format!("Running: {}", slot.ticket_key))
        .unwrap_or_else(|| "No running ticket".to_string());

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(error) = tray.set_title(title) {
            eprintln!("failed to set tray title: {error}");
        }

        if let Err(error) = tray.set_tooltip(Some(tooltip.as_str())) {
            eprintln!("failed to set tray tooltip: {error}");
        }
    }

    if let Some(status) = app.try_state::<TrayStatusState>() {
        if let Err(error) = status.item.set_text(menu_status) {
            eprintln!("failed to set tray status menu item: {error}");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let is_quitting = Arc::new(AtomicBool::new(false));
    let close_is_quitting = Arc::clone(&is_quitting);
    let tray_is_quitting = Arc::clone(&is_quitting);

    let show_tray = nirvana_core::config::AppConfig::load_from_default_path()
        .gui
        .show_tray_icon;

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            if show_tray {
                let status_item = MenuItem::with_id(
                    app,
                    TRAY_STATUS_ID,
                    "No running ticket",
                    false,
                    None::<&str>,
                )?;
                let show_item =
                    MenuItem::with_id(app, TRAY_SHOW_ID, "Show nirvana", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, TRAY_QUIT_ID, "Quit", true, None::<&str>)?;
                let status_separator = PredefinedMenuItem::separator(app)?;
                let action_separator = PredefinedMenuItem::separator(app)?;
                let menu = Menu::with_items(
                    app,
                    &[
                        &status_item,
                        &status_separator,
                        &show_item,
                        &action_separator,
                        &quit_item,
                    ],
                )?;
                app.manage(TrayStatusState {
                    item: status_item.clone(),
                });

                let mut tray = TrayIconBuilder::with_id(TRAY_ID)
                    .tooltip("nirvana")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(move |app, event| match event.id().as_ref() {
                        TRAY_SHOW_ID => show_main_window(app),
                        TRAY_QUIT_ID => {
                            tray_is_quitting.store(true, Ordering::SeqCst);
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main_window(tray.app_handle());
                        }
                    });

                if let Some(icon) = app.default_window_icon().cloned() {
                    tray = tray.icon(icon);
                }

                tray.build(app)?;
                refresh_tray_status(app.handle());
            }

            Ok(())
        })
        .on_window_event(move |window, event| {
            if window.label() != MAIN_WINDOW_LABEL {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                if close_is_quitting.load(Ordering::SeqCst) {
                    return;
                }

                if show_tray {
                    api.prevent_close();

                    if let Err(error) = window.hide() {
                        eprintln!("failed to hide main window: {error}");
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_settings,
            update_settings,
            get_active_connection,
            list_connections,
            set_active_connection,
            delete_connection,
            create_connection,
            list_slots,
            list_unpublished_slots,
            list_recent_tickets,
            get_running_slot,
            start_slot,
            create_slot,
            edit_slot,
            delete_slot,
            stop_slot,
            publish_slots
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        // Show application windows when pressing dock icon with no windows
        if let tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = event
        {
            show_main_window(app);
        }

        // Prevent unused variable warning
        let _ = (app, event);
    });
}
