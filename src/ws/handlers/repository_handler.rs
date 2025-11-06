use std::{collections::HashMap, sync::Arc};

use crate::{
    db,
    ws::{
        ws_message::{
            ClientMessage, ClientMessageType, ErrorCode, ServerMessage, ServerMessageType,
        },
        ws_server::{Client, WsServer},
    },
};
use sqlx::PgPool;
use tokio::sync::Mutex;
use tracing::error;
use uuid::Uuid;

pub async fn repository_handler(
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    id: Uuid,
    pool: PgPool,
    msg: ClientMessage,
) {
    match msg.payload {
        ClientMessageType::AddRepository {
            owner,
            name,
            poll_interval_seconds,
        } => {
            add_repository_handler(
                clients,
                id,
                pool,
                msg.id,
                owner,
                name,
                poll_interval_seconds,
            )
            .await;
        }
        ClientMessageType::UpdateRepository {
            repo_id,
            is_active,
            poll_interval_seconds,
        } => {
            update_repository_handler(
                clients,
                id,
                pool,
                msg.id,
                repo_id,
                is_active,
                poll_interval_seconds,
            )
            .await;
        }
        ClientMessageType::DeleteRepository { repo_id } => {
            delete_repository_handler(clients, id, pool, msg.id, repo_id).await;
        }
        ClientMessageType::GetRepositories {} => {
            get_repositories_handler(clients, id, pool, msg.id).await;
        }
        ClientMessageType::GetRepository { repo_id } => {
            get_repository_handler(clients, id, pool, msg.id, repo_id).await;
        }
        _ => {
            error!("Invalid message: {:?}", msg);
        }
    }
}

pub async fn add_repository_handler(
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    id: Uuid,
    pool: PgPool,
    msg_id: Option<String>,
    owner: String,
    name: String,
    poll_interval_seconds: Option<i32>,
) {
    let repo = match db::Queries::create_repository(&pool, owner, name, poll_interval_seconds).await
    {
        Ok(repo) => repo,
        Err(e) => {
            error!("Failed to add repository: {}", e);
            WsServer::send_error_message(
                clients.clone(),
                id,
                msg_id,
                ErrorCode::DatabaseError,
                "Failed to add repository",
                None,
            )
            .await;
            return;
        }
    };

    let server_message = ServerMessage {
        id: msg_id,
        payload: ServerMessageType::Success {
            message: "Repository added successfully".to_string(),
            data: Some(serde_json::to_value(&repo).unwrap()),
        },
    };

    WsServer::send_message(
        clients.clone(),
        id,
        serde_json::to_string(&server_message).unwrap().as_str(),
    )
    .await;
}

pub async fn get_repositories_handler(
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    id: Uuid,
    pool: PgPool,
    msg_id: Option<String>,
) {
    let repo_list = match db::Queries::get_repositories(&pool).await {
        Ok(repo_list) => repo_list,
        Err(e) => {
            error!("Failed to get repository list: {}", e);
            WsServer::send_error_message(
                clients.clone(),
                id,
                msg_id,
                ErrorCode::DatabaseError,
                "Failed to get repository list",
                None,
            )
            .await;
            return;
        }
    };

    let server_message = ServerMessage {
        id: msg_id,
        payload: ServerMessageType::Repositories {
            repositories: repo_list
                .into_iter()
                .map(|repo| serde_json::to_value(&repo).unwrap())
                .collect(),
        },
    };

    WsServer::send_message(
        clients.clone(),
        id,
        serde_json::to_string(&server_message).unwrap().as_str(),
    )
    .await;
}

pub async fn get_repository_handler(
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    id: Uuid,
    pool: PgPool,
    msg_id: Option<String>,
    repo_id: i32,
) {
    let repo = match db::Queries::get_repository(&pool, repo_id).await {
        Ok(repo) => repo,
        Err(e) => {
            error!("Failed to get repository: {}", e);
            WsServer::send_error_message(
                clients.clone(),
                id,
                msg_id,
                ErrorCode::DatabaseError,
                "Failed to get repository",
                None,
            )
            .await;
            return;
        }
    };

    let server_message = ServerMessage {
        id: msg_id,
        payload: ServerMessageType::Repository {
            repository: serde_json::to_value(&repo).unwrap(),
        },
    };

    WsServer::send_message(
        clients.clone(),
        id,
        serde_json::to_string(&server_message).unwrap().as_str(),
    )
    .await;
}

pub async fn update_repository_handler(
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    id: Uuid,
    pool: PgPool,
    msg_id: Option<String>,
    repo_id: i32,
    is_active: Option<bool>,
    poll_interval_seconds: Option<i32>,
) {
    let repo = match db::Queries::update_repository(
        &pool,
        repo_id,
        is_active,
        poll_interval_seconds,
    )
    .await
    {
        Ok(repo) => repo,
        Err(e) => {
            error!("Failed to update repository: {}", e);
            WsServer::send_error_message(
                clients.clone(),
                id,
                msg_id,
                ErrorCode::DatabaseError,
                "Failed to update repository",
                None,
            )
            .await;
            return;
        }
    };

    let server_message = ServerMessage {
        id: msg_id,
        payload: ServerMessageType::Success {
            message: "Repository updated successfully".to_string(),
            data: Some(serde_json::to_value(&repo).unwrap()),
        },
    };

    WsServer::send_message(
        clients.clone(),
        id,
        serde_json::to_string(&server_message).unwrap().as_str(),
    )
    .await;
}

pub async fn delete_repository_handler(
    clients: Arc<Mutex<HashMap<Uuid, Client>>>,
    id: Uuid,
    pool: PgPool,
    msg_id: Option<String>,
    repo_id: i32,
) {
    if db::Queries::delete_repository(&pool, repo_id)
        .await
        .is_err()
    {
        error!("Failed to delete repository");
        WsServer::send_error_message(
            clients.clone(),
            id,
            msg_id,
            ErrorCode::DatabaseError,
            "Failed to delete repository",
            None,
        )
        .await;
        return;
    };

    let server_message = ServerMessage {
        id: msg_id,
        payload: ServerMessageType::Success {
            message: "Repository deleted successfully".to_string(),
            data: None,
        },
    };

    WsServer::send_message(
        clients.clone(),
        id,
        serde_json::to_string(&server_message).unwrap().as_str(),
    )
    .await;
}
