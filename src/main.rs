use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
    routing::{get, post},
    Router,
};
use rocksdb::{TransactionDB, TransactionDBOptions};
use tokio::sync::Notify;
use ulid::Ulid;

async fn ingest(State(state): State<Arc<AppState>>, data: Bytes) {
    let key = Ulid::new();
    state
        .db
        .put(/* key.0.to_be_bytes() */ key.to_string(), data)
        .unwrap();
    state.notifier.notify_one();
}

#[derive(serde::Serialize)]
struct ListenMessage {
    key: String,
    value: String,
}

async fn listen(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(|socket| listen_handler(socket, state))
}

async fn listen_handler(mut socket: WebSocket, state: Arc<AppState>) {
    // First clear out any existing data, then watch for new data to come in
    loop {
        let mut iter = state.db.iterator(rocksdb::IteratorMode::Start);
        while let Some(entry) = iter.next() {
            let (key, value) = entry.unwrap();

            let transaction = state.db.transaction();
            if transaction.get_for_update(&key, true).unwrap().is_none() {
                continue;
            }
            transaction.delete(&key).unwrap();

            let key = std::str::from_utf8(&key).unwrap();
            let value = std::str::from_utf8(&value).unwrap();
            if let Err(err) = socket.send(value.into()).await {
                eprintln!("error sending: {} {}", key, err);
                transaction.rollback().unwrap();
                return;
            }

            if let Some(response) = socket.recv().await {
                if let Ok(response) = response {
                    if response.to_text().unwrap() == "ACK" {
                        transaction.commit().unwrap();
                        continue;
                    } else {
                        eprintln!("error confirming receipt of {}", key);
                    }
                } else {
                    eprintln!("error confirming receipt of {}", key);
                }
            } else {
                eprintln!("socket closed");
            }

            transaction.rollback().unwrap();
        }
        drop(iter);

        state.notifier.notified().await;
    }
}

struct AppState {
    db: TransactionDB,
    notifier: Notify,
}

#[tokio::main]
async fn main() {
    let mut transaction_options = TransactionDBOptions::default();
    transaction_options.set_default_lock_timeout(-1);
    transaction_options.set_txn_lock_timeout(-1);

    let db =
        TransactionDB::open(&rocksdb::Options::default(), &transaction_options, "test").unwrap();

    let notifier = tokio::sync::Notify::new();

    let state = Arc::new(AppState { db, notifier });

    let app = Router::new()
        .route("/ingest", post(ingest))
        .route("/listen", get(listen))
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:9781".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
