mod config;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
    use dotenv::dotenv;
use tokio::sync::Mutex;
use axum::extract::ConnectInfo;
use serde::{Serialize,Deserialize};
use chrono::Utc;
use axum::http::HeaderMap;
use axum::extract::Multipart;
use axum::http::StatusCode;
use uuid::Uuid;
use std::path::Path;
use tokio::fs;
use axum::extract::DefaultBodyLimit;
use config::{Config, load_config};



#[derive(Clone)]
struct AppState {
    tx: Arc<broadcast::Sender<String>>,
    messages: Arc<Mutex<Vec<ChatMessage>>>, // Store last 50 messages
    config: Arc<Config>, // Load config file
}

#[derive(Clone, Serialize)]
enum Role {
    Admin,
    User,
}
#[derive(Clone, Serialize)]
struct Member{
    role: Role,
    name: String,
    ip: String,
}


#[derive(Clone, Serialize, Deserialize)]
struct Attachment {
    url: String,
    filename: String,
    mime: String,
}

#[derive(Clone, Deserialize)]
struct IncomingMessage {
    text: Option<String>,
    attachment: Option<Attachment>,
}


#[derive(Clone, Serialize)]
struct ChatMessage {
    text: String,
    time: String,
    username: String,
    role: Role,
    r#type: String, // "join" | "leave" | "message"
    #[serde(skip_serializing_if = "Option::is_none")]
    attachment: Option<Attachment>,
}



#[tokio::main]
async fn main() {
    dotenv().ok();
    let config = Arc::new(load_config());
    let (tx, _rx) = broadcast::channel::<String>(100);

    let state = AppState {
        tx: Arc::new(tx),
        messages: Arc::new(Mutex::new(Vec::new())),
        config: config.clone(),
    };

    let static_files = ServeDir::new("static");

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/upload", axum::routing::post(upload_handler))
        .fallback_service(static_files)
        .layer(DefaultBodyLimit::max(state.config.body_limit_bytes))
        .with_state(state.clone());


    let bind_addr = state.config.bind_addr.clone();

    let addr: SocketAddr = bind_addr
        .parse()
        .expect("BIND_ADDR must be a valid socket address");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    println!("listening on http://{}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
        .await
        .unwrap();


}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.split(',').next().unwrap().trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    ws.on_upgrade(move |socket| handle_socket(socket, state, ip))
}
async fn handle_socket(socket: WebSocket, state: AppState, ip : String) {

    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Task responsible for sending broadcast messages to this socket
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive username (first message)
    let username = match receiver.next().await {
        Some(Ok(Message::Text(name))) => name,
        _ => return,
    };

    let parts: Vec<&str> = username.split("::").collect();

    let mut member = Member {
        name: username.to_string(),
        role: Role::User,
        ip,
    };
    if member.name == "system" {
        member.name = "[system]".to_string();
    }

    // Admin token validation
    if parts.len() == 2
        && parts[1]
        == state.config.token.as_str()
    {
        member.role = Role::Admin;
        member.name = parts[0].to_string();
    }

    // Send last 50 messages to the new user as a JSON array
    {
        let messages = state.messages.lock().await;
        let json = serde_json::to_string(&*messages).unwrap();
        let _ = state.tx.send(json);
    }

    // User joined message
    broadcast_message(
        &state,
        ChatMessage {
            text: format!("{} joined", member.name),
            time: Utc::now().to_rfc3339(),
            username: "system".to_string(),
            role: member.role.clone(),
            r#type: "join".into(),
            attachment: None,
        },
    )
        .await;

    // Receive user messages
    while let Some(Ok(Message::Text(raw))) = receiver.next().await {
        let (text, attachment) = match serde_json::from_str::<IncomingMessage>(&raw) {
            Ok(v) => (v.text.unwrap_or_default(), v.attachment),
            Err(_) => (raw.to_string(), None),
        };

        broadcast_message(
            &state,
            ChatMessage {
                text,
                time: Utc::now().to_rfc3339(),
                username: member.name.clone(),
                role: member.role.clone(),
                r#type: "message".into(),
                attachment,
            },
        )
            .await;
    }

    // User left message
    broadcast_message(
        &state,
        ChatMessage {
            text: format!("{} left", member.name),
            time: Utc::now().to_rfc3339(),
            username: "system".to_string(),
            role: member.role.clone(),
            r#type: "leave".into(),
            attachment: None,
        },
    )
        .await;

    send_task.abort();
}

fn attachment_disk_path_from_url(url: &str) -> Option<String> {
    // Covers both /uploads/xxx and http(s)://.../uploads/xxx
    let marker = "/uploads/";
    let idx = url.find(marker)?;
    let rel = &url[idx + marker.len()..];
    if rel.is_empty() {
        return None;
    }
    Some(format!("static/uploads/{}", rel))
}


// Broadcast messages as JSON ARRAY and keep only last 50
async fn broadcast_message(state: &AppState, msg: ChatMessage) {
    // dont send msg or file when are empty
    if msg.text.trim().is_empty() && msg.attachment.is_none() {
        return;
    }

    let mut to_delete: Vec<String> = vec![];

    if msg.username != "system" {
        let mut messages = state.messages.lock().await;
        messages.push(msg.clone());
        let limit = state.config.message_limit;


        // Delete last 50 msg
        if limit > 0 {
            while messages.len() > limit {
                let removed = messages.remove(0);
                if let Some(att) = removed.attachment {
                    if let Some(p) = attachment_disk_path_from_url(&att.url) {
                        to_delete.push(p);
                    }
                }
            }
        }

    }

    //  delete files outs of Lock
    for p in to_delete {
        match fs::remove_file(&p).await {
            Ok(_) => println!("Deleted upload file: {}", p),
            Err(e) => eprintln!("Failed to delete upload file {}: {}", p, e),
        }
    }


    let json = serde_json::to_string(&vec![msg]).unwrap();
    let _ = state.tx.send(json);
}


async fn upload_handler(
    State(state):State<AppState>,
    mut multipart: Multipart ) -> impl IntoResponse {

    let allowed = &state.config.allowed_mimes;
    let max_bytes = state.config.max_upload_bytes;
    let upload_dir = state.config.upload_dir.clone();

    while let Some(field) = multipart.next_field().await.ok().flatten() {
        if field.name() != Some("file") {
            continue;
        }

        let filename = field.file_name().unwrap_or("file").to_string();
        let mime = field.content_type().unwrap_or("application/octet-stream").to_string();

        if !allowed.contains(&mime) {
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "MIME not allowed").into_response();
        }

        let mut data = Vec::new();
        let mut field_stream = field;

        while let Some(chunk) = field_stream.chunk().await.unwrap_or(None) {
            data.extend_from_slice(&chunk);

            if data.len() > max_bytes {
                return (StatusCode::PAYLOAD_TOO_LARGE, "File too large").into_response();
            }
        }


        let dir = Path::new(&upload_dir);
        if fs::create_dir_all(dir).await.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "failed to create upload dir").into_response();
        }

        let ext = Path::new(&filename).extension().and_then(|e| e.to_str()).unwrap_or("");

        let saved = if ext.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            format!("{}.{}", Uuid::new_v4(), ext)
        };

        let path = dir.join(&saved);
        if fs::write(&path, data).await.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "failed to save file").into_response();
        }

        let body = serde_json::json!({
            "url": format!("/uploads/{}", saved),
            "filename": filename,
            "mime": mime
        });

        return axum::Json(body).into_response();
    }

    (StatusCode::BAD_REQUEST, "No file").into_response()
}
