use crate::ws::handlers::repository_handler::repository_handler;
use crate::ws::ws_message::{
    ClientMessage, ClientMessageType, ErrorCode, ServerMessage, ServerMessageType,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{self, Value};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpListener;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};
use tracing::{error, info};
use uuid::Uuid;

type WebSocketWriter = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>;

#[derive(Debug, Clone)]
pub struct Client {
    /// WebSocket 쓰기 스트림
    pub write: Arc<Mutex<WebSocketWriter>>,
}

pub struct WsServer {
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    pool: PgPool,
}

impl WsServer {
    pub fn new(pool: &PgPool) -> Self {
        WsServer {
            clients: Arc::new(Mutex::new(HashMap::new())),
            pool: pool.clone(),
        }
    }

    pub async fn run(&self, port: u16) {
        let addr = format!("0.0.0.0:{}", port);
        info!("WebSocket Server Start: {}", addr);
        let listener = TcpListener::bind(addr).await.unwrap();

        while let Ok((stream, addr)) = listener.accept().await {
            // WebSocket 연결로 업그레이드
            let ws_stream = match accept_async(stream).await {
                Ok(ws_stream) => ws_stream,
                Err(e) => {
                    error!("WebSocket upgrade failed: {}", e);
                    break;
                }
            };

            let id = uuid::Uuid::new_v4();
            info!("New Connection: {}, {}", addr, id);

            let (write, read) = ws_stream.split();
            self.clients.lock().await.insert(
                id,
                Client {
                    write: Arc::new(Mutex::new(write)),
                },
            );

            // 클라이언트별 메시지 처리 태스크 시작
            let pool = self.pool.clone();
            let clients = self.clients.clone();
            tokio::spawn(async move {
                let mut read_stream = read;
                loop {
                    let msg = match read_stream.next().await {
                        Some(Ok(message)) => message,
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            info!("Client disconnected: {}", id);
                            break;
                        }
                    };

                    let msg_text = match msg {
                        Message::Text(text) => text,
                        Message::Binary(data) => {
                            error!("Unexpected binary data: {} bytes {:02X?}", data.len(), data);
                            continue;
                        }
                        Message::Ping(data) => {
                            // Ping에 대해 Pong으로 응답 (연결 상태 확인)
                            if let Some(client) = clients.lock().await.get_mut(&id) {
                                let mut write_lock = client.write.lock().await;
                                if let Err(e) = write_lock.send(Message::Pong(data)).await {
                                    error!("Failed to send pong: {}", e);
                                    break; // 연결에 문제가 있으면 루프 종료
                                }
                            }
                            continue;
                        }
                        Message::Pong(_) => {
                            // Pong은 단순히 무시 (연결 상태 확인 완료)
                            continue;
                        }
                        Message::Close(_) => {
                            // 클라이언트가 연결을 닫으면 루프 종료
                            clients.lock().await.remove(&id);
                            info!("Client {} requested connection close", id);
                            break;
                        }
                        Message::Frame(_) => continue,
                    };

                    info!("Received message: {}", msg_text);
                    let msg_parsed = match serde_json::from_str::<ClientMessage>(&msg_text) {
                        Ok(msg) => msg,
                        Err(e) => {
                            error!("Message parse to json error: {}, {}", e, msg_text);
                            WsServer::send_message(
                                clients.clone(),
                                id,
                                serde_json::to_string("{\"error\": \"Invalid message\"}")
                                    .unwrap()
                                    .as_str(),
                            )
                            .await;
                            continue;
                        }
                    };

                    match msg_parsed.payload {
                        ClientMessageType::AddRepository { .. }
                        | ClientMessageType::UpdateRepository { .. }
                        | ClientMessageType::DeleteRepository { .. }
                        | ClientMessageType::GetRepositories { .. }
                        | ClientMessageType::GetRepository { .. } => {
                            repository_handler(clients.clone(), id, pool.clone(), msg_parsed).await;
                        }
                        _ => {
                            continue;
                        }
                    }
                }
            });
        }
    }

    pub async fn send_message(clients: Arc<Mutex<HashMap<Uuid, Client>>>, id: Uuid, message: &str) {
        let write = {
            let guard = clients.lock().await;
            if let Some(client) = guard.get(&id) {
                client.write.clone()
            } else {
                return; // 클라이언트가 존재하지 않으면 종료
            }
        }; // clients 락 해제

        let mut wl = write.lock().await;
        if let Err(e) = wl.send(Message::text(message)).await {
            error!("send message error: {}", e);
        }
    }

    pub async fn send_error_message(
        clients: Arc<Mutex<HashMap<Uuid, Client>>>,
        id: Uuid,
        msg_id: Option<String>,
        code: ErrorCode,
        message: &str,
        details: Option<Value>,
    ) {
        let server_message = ServerMessage {
            id: msg_id,
            payload: ServerMessageType::Error {
                code: code,
                message: message.to_string(),
                details: details,
            },
        };

        WsServer::send_message(
            clients,
            id,
            serde_json::to_string(&server_message).unwrap().as_str(),
        )
        .await;
    }
}
