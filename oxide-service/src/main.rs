use local_ip_address::local_ip;
use oxide_core::app::AppState;
use oxide_core::room_manager::RoomManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Debug)]
enum Command {
    ListRooms,
    CreateRoom {
        name: String,
        peer_id: String,
        alias: String,
        real_ip: String,
        port: u16,
        max_players: u32,
    },
    JoinRoom {
        id: String,
        peer_id: String,
        alias: String,
        real_ip: String,
        port: u16,
    },
    LeaveRoom {
        id: String,
        peer_id: String,
    },
    RoomInfo {
        id: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
enum Response {
    Rooms(Vec<oxide_core::room::Room>),
    Room(oxide_core::room::Room),
    Success(String),
    Error(String),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let my_ip = local_ip().unwrap_or_else(|_| "0.0.0.0".parse().unwrap());

    let state = Arc::new(AppState::new(my_ip.to_string(), "Daemon".to_string()));

    let room_manager = Arc::new(Mutex::new(
        RoomManager::new("Daemon".to_string())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize room manager: {}", e))?,
    ));

    println!("Oxide Service starting on {}", my_ip);

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Listening on 127.0.0.1:8080");

    loop {
        let (socket, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        let room_manager = Arc::clone(&room_manager);

        tokio::spawn(async move {
            handle_connection(socket, state, room_manager).await;
        });
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    _state: Arc<AppState>,
    room_manager: Arc<Mutex<RoomManager>>,
) {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while let Ok(bytes_read) = reader.read_line(&mut line).await {
        if bytes_read == 0 {
            break;
        }

        let command: Result<Command, _> = serde_json::from_str(&line.trim());
        let response = match command {
            Ok(Command::ListRooms) => {
                let rooms = room_manager.lock().await.list_rooms();
                Response::Rooms(rooms)
            }
            Ok(Command::CreateRoom {
                name,
                peer_id,
                alias,
                real_ip,
                port,
                max_players,
            }) => {
                let mut rm = room_manager.lock().await;
                match rm
                    .create_room(name.clone(), peer_id, alias, real_ip, port, max_players)
                    .await
                {
                    Ok(room) => {
                        Response::Success(format!("Room '{}' created with ID {}", name, room.id))
                    }
                    Err(e) => Response::Error(format!("Failed to create room: {}", e)),
                }
            }
            Ok(Command::JoinRoom {
                id,
                peer_id,
                alias,
                real_ip,
                port,
            }) => {
                let mut rm = room_manager.lock().await;
                match rm.join_room(&id, peer_id, alias, real_ip, port).await {
                    Ok(_) => Response::Success(format!("Joined room {}", id)),
                    Err(e) => Response::Error(format!("Failed to join room: {}", e)),
                }
            }
            Ok(Command::LeaveRoom { id, peer_id }) => {
                let mut rm = room_manager.lock().await;
                match rm.leave_room(&id, &peer_id).await {
                    Ok(_) => Response::Success(format!("Left room {}", id)),
                    Err(e) => Response::Error(format!("Failed to leave room: {}", e)),
                }
            }
            Ok(Command::RoomInfo { id }) => {
                let rm = room_manager.lock().await;
                match rm.get_room(&id) {
                    Some(room) => Response::Room(room),
                    None => Response::Error(format!("Room {} not found", id)),
                }
            }
            Err(e) => Response::Error(format!("Invalid command: {}", e)),
        };

        let response_json = serde_json::to_string(&response).unwrap() + "\n";
        if writer.write_all(response_json.as_bytes()).await.is_err() {
            break;
        }
        writer.flush().await.unwrap();
        line.clear();
    }
}
