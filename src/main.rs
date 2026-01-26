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
    use std::env;
use tokio::sync::Mutex;
use axum::extract::ConnectInfo;
use serde::{Serialize};
use chrono::Utc;
use axum::http::HeaderMap;

#[derive(Clone)]
struct AppState {
    tx: Arc<broadcast::Sender<String>>,
    messages: Arc<Mutex<Vec<ChatMessage>>>, // Store last 50 messages
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

#[derive(Clone, Serialize)]
struct ChatMessage {
    text: String,
    time: String,
    username: String,
    role: Role,
    r#type: String, // "join" | "leave" | "message"
}
#[tokio::main]
async fn main() {
    dotenv().ok();

    let (tx, _rx) = broadcast::channel::<String>(100);

    let state = AppState {
        tx: Arc::new(tx),
        messages: Arc::new(Mutex::new(Vec::new())),
    };

    let static_files = ServeDir::new("static");

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(static_files)
        .with_state(state);


    let bind_addr = env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string());

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
        == env::var("TOKEN").unwrap_or_else(|_| "token".to_string())
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
        },
    )
        .await;

    // Receive user messages
    while let Some(Ok(Message::Text(msg))) = receiver.next().await {
        broadcast_message(
            &state,
            ChatMessage {
                text: msg.to_string(),
                time: Utc::now().to_rfc3339(),
                username: member.name.clone(),
                role: member.role.clone(),
                r#type: "message".into(),
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
        },
    )
        .await;

    send_task.abort();
}

// Broadcast messages as JSON ARRAY and keep only last 50
async fn broadcast_message(state: &AppState, msg: ChatMessage) {

    if msg.text.trim().is_empty() {
        return;
    }
    if msg.username != "system" {

        let mut messages = state.messages.lock().await;
        messages.push(msg.clone());
        if messages.len() > 50 {
            messages.remove(0);
        }
        // Keep only last 50 messages
    }

    // Always send messages as an array
    let json = serde_json::to_string(&vec![msg]).unwrap();
    let _ = state.tx.send(json);
}

