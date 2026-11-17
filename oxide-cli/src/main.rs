use clap::{Parser, Subcommand};
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Parser)]
#[command(name = "oxide-cli")]
#[command(about = "Oxide VPN CLI client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Address of the Oxide service to use
    #[arg(short, long, default_value = "127.0.0.1:8080", global = true)]
    service: String,
}

#[derive(Subcommand)]
enum Commands {
    /// List available rooms
    List,
    /// Create a new room
    Create {
        /// Room name
        name: String,
        /// Alias to use for this client
        #[arg(short, long)]
        alias: Option<String>,
        /// Maximum number of players for the created room
        #[arg(short, long, default_value_t = 5)]
        max_players: u32,
    },
    /// Join an existing room
    Join {
        /// Room ID
        id: String,
        /// Alias to use for this client
        #[arg(short, long)]
        alias: Option<String>,
    },
    /// Leave a room by ID
    Leave {
        /// Room ID
        id: String,
    },
    /// Bring the VPN tunnel up (needs root on Linux, Administrator on Windows).
    ///
    /// Host (hub):   oxide-cli up --ip 10.0.0.1 --hub
    /// Friend:       oxide-cli up --ip 10.0.0.2 --connect HOST_PUBLIC_IP:9000
    Up {
        /// Your virtual IP, unique per player (10.0.0.1 - 10.0.0.254)
        #[arg(long)]
        ip: String,
        /// Hub address to connect to (host:port). Omit on the hub itself.
        #[arg(long)]
        connect: Option<String>,
        /// Act as the hub: relay traffic between all players
        #[arg(long)]
        hub: bool,
        /// Local UDP port (the hub must forward this port on the router)
        #[arg(long, default_value_t = 9000)]
        port: u16,
    },
    /// Show room details and peer status
    Status {
        /// Room ID
        id: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct CliConfig {
    peer_id: String,
    alias: String,
    port: u16,
}

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

async fn load_or_create_config() -> anyhow::Result<(CliConfig, PathBuf)> {
    let config_dir = get_config_dir()?;
    fs::create_dir_all(&config_dir).await?;

    let config_path = config_dir.join("oxide-cli.json");
    if fs::try_exists(&config_path).await? {
        let content = fs::read_to_string(&config_path).await?;
        let config: CliConfig = serde_json::from_str(&content)?;
        return Ok((config, config_path));
    }

    let config = CliConfig {
        peer_id: generate_peer_id(),
        alias: default_alias(),
        port: 9000,
    };

    let json = serde_json::to_string_pretty(&config)?;
    fs::write(&config_path, json).await?;
    Ok((config, config_path))
}

fn default_alias() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "OxideUser".to_string())
}

fn get_config_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA")?;
        Ok(PathBuf::from(app_data).join("Oxide"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home).join("Library/Application Support/Oxide"))
    }

    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home).join(".config/Oxide"))
    }
}

fn generate_peer_id() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("cli-{}", now)
}

async fn save_config(config: &CliConfig, path: &PathBuf) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Commands::Up {
        ip,
        connect,
        hub,
        port,
    } = &cli.command
    {
        return run_vpn(ip, connect.as_deref(), *hub, *port).await;
    }

    let (mut cli_config, config_path) = load_or_create_config().await?;

    let local_ip = local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "0.0.0.0".to_string());

    let command = match cli.command {
        Commands::List => Command::ListRooms,
        Commands::Create {
            name,
            alias,
            max_players,
        } => {
            let alias = alias.unwrap_or_else(|| cli_config.alias.clone());
            if alias != cli_config.alias {
                cli_config.alias = alias.clone();
                save_config(&cli_config, &config_path).await?;
            }
            Command::CreateRoom {
                name,
                peer_id: cli_config.peer_id.clone(),
                alias,
                real_ip: local_ip.clone(),
                port: cli_config.port,
                max_players,
            }
        }
        Commands::Join { id, alias } => {
            let alias = alias.unwrap_or_else(|| cli_config.alias.clone());
            if alias != cli_config.alias {
                cli_config.alias = alias.clone();
                save_config(&cli_config, &config_path).await?;
            }
            Command::JoinRoom {
                id,
                peer_id: cli_config.peer_id.clone(),
                alias,
                real_ip: local_ip.clone(),
                port: cli_config.port,
            }
        }
        Commands::Leave { id } => Command::LeaveRoom {
            id,
            peer_id: cli_config.peer_id.clone(),
        },
        Commands::Status { id } => Command::RoomInfo { id },
        Commands::Up { .. } => unreachable!("handled above"),
    };

    let mut stream = TcpStream::connect(&cli.service).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to connect to Oxide service at {}: {}. Make sure oxide-service is running and reachable.",
            cli.service,
            e
        )
    })?;

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    let command_json = serde_json::to_string(&command)? + "\n";
    writer.write_all(command_json.as_bytes()).await?;
    writer.flush().await?;

    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;
    let response: Response = serde_json::from_str(&response_line.trim())?;

    match response {
        Response::Rooms(rooms) => {
            println!("Available rooms:");
            for room in rooms {
                println!(
                    " - {} ({}) [{} players]",
                    room.id,
                    room.name,
                    room.peers.len()
                );
            }
        }
        Response::Room(room) => {
            println!("Room {} - {}", room.id, room.name);
            println!("Creator: {}", room.creator_id);
            println!("Peers: {} / {}", room.peers.len(), room.max_players);
            for peer in room.peers.values() {
                println!(
                    " - {} ({}) {} status={:?} last_seen={}",
                    peer.id,
                    peer.alias,
                    peer.socket_addr(),
                    peer.status,
                    peer.last_seen
                );
            }
        }
        Response::Success(msg) => println!("{}", msg),
        Response::Error(msg) => eprintln!("Error: {}", msg),
    }

    Ok(())
}

async fn run_vpn(ip: &str, connect: Option<&str>, hub: bool, port: u16) -> anyhow::Result<()> {
    use oxide_core::app::AppState;
    use oxide_core::network::vpn_tunnel::VpnTunnel;
    use oxide_core::system::tun_device::TunDevice;
    use std::net::ToSocketAddrs;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    if !hub && connect.is_none() {
        anyhow::bail!("use --hub (you host) or --connect HOST:PORT (you join)");
    }
    let seeds: Vec<std::net::SocketAddr> = match connect {
        Some(addr) => vec![addr
            .to_socket_addrs()?
            .find(|a| a.is_ipv4())
            .ok_or_else(|| anyhow::anyhow!("cannot resolve {}", addr))?],
        None => vec![],
    };

    let state = Arc::new(AppState::new("0.0.0.0".into(), "cli".into()));
    state.echo_stdout.store(true, Ordering::Relaxed);
    let tun = TunDevice::new(ip)?;
    let socket = Arc::new(tokio::net::UdpSocket::bind(("0.0.0.0", port)).await?);
    let mut tunnel = VpnTunnel::with_peers(tun, socket, state.clone(), ip, seeds)
        .await?
        .set_hub(hub);

    let shutdown = state.shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        shutdown.store(true, Ordering::Relaxed);
    });

    tunnel.run().await
}
