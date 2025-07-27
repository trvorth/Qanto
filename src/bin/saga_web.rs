use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt}; // <-- ADDED THIS LINE
use qanto::saga::{NetworkState, PalletSaga};
use std::sync::Arc;
// use tokio::sync::Mutex; // <-- REMOVED THIS UNUSED IMPORT
use tower_http::services::ServeDir;

// AppState holds the shared SAGA instance
struct AppState {
    saga: PalletSaga,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // The ui folder should be at the root of your project, next to 'src'
    // For this example, we assume the 'ui' folder from the zip is at the root.
    let serve_dir = ServeDir::new("ui");

    let app_state = Arc::new(AppState {
        saga: PalletSaga::new(
            #[cfg(feature = "infinite-strata")]
            None,
        ),
    });

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .fallback_service(serve_dir)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::info!("✓ SAGA Web Console listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    // Split the socket into a sender and receiver.
    let (mut sender, mut receiver) = stream.split();
    let saga_guidance = state.saga.guidance_system.clone();

    // Loop and process messages from the client
    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(query) = message {
            // Use the existing guidance system to process the query
            let response = match saga_guidance
                .get_guidance_response(
                    &query,
                    NetworkState::Nominal, // Mock state for now
                    qanto::omega::identity::ThreatLevel::Nominal,
                    None,
                )
                .await
            {
                Ok(resp) => resp,
                Err(e) => format!("SAGA Error: {e}"),
            };

            // Send the response back to the client
            if sender.send(Message::Text(response)).await.is_err() {
                // Client disconnected
                break;
            }
        }
    }
}
