use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Extension, Router,
};
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;
use crate::simulation::Simulation;

pub async fn run_server(sim: Arc<Mutex<Simulation>>, port: u16) {
    let app = Router::new()
        .fallback_service(ServeDir::new("static"))
        .route("/ws", get(ws_handler))
        .layer(Extension(sim));

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    println!("Server running on http://localhost:{}", port);
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(sim): Extension<Arc<Mutex<Simulation>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, sim))
}

async fn handle_socket(mut socket: WebSocket, sim: Arc<Mutex<Simulation>>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    loop {
        interval.tick().await;
        let state = {
            let sim_lock = sim.lock().unwrap();
            sim_lock.to_state()
        };
        
        if let Ok(json) = serde_json::to_string(&state) {
            if socket.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    }
}
