use nirvana_core::api::domain::{
    AppSettings, Change, Connection, ConnectionData, PublishFailure, PublishResult, Slot,
    SlotCreate, SlotEdit, Ticket,
};
use nirvana_core::api::{NirvanaApi, SlotSort};
use serde::{Deserialize, Serialize};

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
        }
    }
}

impl From<GuiSettings> for AppSettings {
    fn from(settings: GuiSettings) -> Self {
        Self {
            publish_squashed_worklogs: settings.publish_squashed_worklogs,
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
fn set_active_connection(input: ConnectionIdInput) -> Result<Option<GuiConnection>, String> {
    let mut api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.set_active_connection(input.connection_id)
        .map_err(|error| error.to_string())?;
    api.get_active_connection()
        .map(|connection| connection.map(GuiConnection::from))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_connection(input: ConnectionIdInput) -> Result<Option<GuiConnection>, String> {
    let mut api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.delete_connection(input.connection_id)
        .map_err(|error| error.to_string())?;
    api.get_active_connection()
        .map(|connection| connection.map(GuiConnection::from))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_connection(input: CreateConnectionInput) -> Result<GuiConnection, String> {
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
fn list_recent_tickets() -> Result<Vec<GuiTicket>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.list_recent_tickets()
        .map(|tickets| tickets.into_iter().map(GuiTicket::from).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_slot(input: StartSlotInput) -> Result<GuiSlot, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.start_slot(&input.ticket_key, input.started_at, input.note.as_deref())
        .map(GuiSlot::from)
        .map_err(|error| error.to_string())
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
fn delete_slot(input: DeleteSlotInput) -> Result<GuiSlot, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.delete_slot(input.slot_id)
        .map(GuiSlot::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn stop_slot() -> Result<Option<GuiSlot>, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.stop_slot(None)
        .map(|slot| slot.map(GuiSlot::from))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn publish_slots(input: PublishSlotsInput) -> Result<GuiPublishResult, String> {
    let api = NirvanaApi::new().map_err(|error| error.to_string())?;
    api.publish(input.from, input.to)
        .map(GuiPublishResult::from)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            list_recent_tickets,
            start_slot,
            create_slot,
            edit_slot,
            delete_slot,
            stop_slot,
            publish_slots
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
