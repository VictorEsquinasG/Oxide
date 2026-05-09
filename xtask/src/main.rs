use anyhow::Result;
use clap::{Parser, Subcommand};
use xshell::{cmd, Shell};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Build and deployment helpers for Oxide")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build portable release bundle
    Portable {
        /// Output directory for portable bundle
        #[arg(short, long, default_value = "dist")]
        output: String,

        /// Build in release mode (default: debug)
        #[arg(short, long)]
        release: bool,

        /// Create ZIP archive after building
        #[arg(short, long)]
        zip: bool,
    },

    /// Build release binaries
    Release {
        /// Target triple (leave empty for native)
        #[arg(short, long)]
        target: Option<String>,
    },

    /// Run GUI locally for testing
    DevGui,

    /// Run service daemon locally for testing
    DevService,

    /// Clean all build artifacts
    Clean,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let sh = Shell::new()?;

    match cli.command {
        Commands::Portable {
            output,
            release,
            zip,
        } => {
            build_portable(&sh, &output, release, zip)?;
        }
        Commands::Release { target } => {
            build_release(&sh, target)?;
        }
        Commands::DevGui => {
            dev_gui(&sh)?;
        }
        Commands::DevService => {
            dev_service(&sh)?;
        }
        Commands::Clean => {
            clean(&sh)?;
        }
    }

    Ok(())
}

fn build_portable(sh: &Shell, output_dir: &str, release: bool, zip: bool) -> Result<()> {
    println!("🔨 Building portable Oxide bundle...\n");

    // Create output structure
    let bundle_path = format!("{}/Oxide-Portable", output_dir);
    cmd!(sh, "mkdir -p {bundle_path}").run()?;
    cmd!(sh, "mkdir -p {bundle_path}/config").run()?;
    cmd!(sh, "mkdir -p {bundle_path}/assets").run()?;

    // Determine build profile
    let profile_flag = if release { "--release" } else { "" };
    let profile_name = if release { "release" } else { "debug" };

    println!("📦 Building oxide-gui...");
    cmd!(sh, "cargo build {profile_flag} -p oxide-gui").run()?;

    println!("📦 Building oxide-service...");
    cmd!(sh, "cargo build {profile_flag} -p oxide-service").run()?;

    // Copy binaries
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let gui_src = format!("target/{profile_name}/oxide-gui{ext}");
    let service_src = format!("target/{profile_name}/oxide-service{ext}");

    println!("📁 Copying binaries...");
    cmd!(sh, "cp {gui_src} {bundle_path}/oxide{ext}").run()?;
    cmd!(sh, "cp {service_src} {bundle_path}/oxide-service{ext}").run()?;

    // Copy assets
    println!("📁 Copying assets...");
    if std::path::Path::new("assets/Icon.png").exists() {
        cmd!(sh, "cp assets/Icon.png {bundle_path}/assets/").run()?;
    }

    // Create README
    println!("📝 Creating portable README...");
    create_portable_readme(&bundle_path)?;

    // Create platform-specific launcher
    if cfg!(windows) {
        create_windows_launcher(&bundle_path)?;
    } else {
        create_unix_launcher(&bundle_path)?;
    }

    println!("✅ Portable bundle created at: {}\n", bundle_path);

    // Optional: Create ZIP archive
    if zip {
        println!("📦 Creating ZIP archive...");
        let zip_name = "Oxide-Portable";
        cmd!(
            sh,
            "cd {output_dir} && tar -czf {zip_name}.tar.gz Oxide-Portable/"
        )
        .run()?;
        println!(
            "✅ ZIP archive created: {}/{}.tar.gz\n",
            output_dir, zip_name
        );

        if cfg!(windows) {
            // For Windows, also try PowerShell compression if available
            let _ = cmd!(sh, "powershell -Command \"Compress-Archive -Path {output_dir}\\Oxide-Portable -DestinationPath {output_dir}\\{zip_name}.zip\"").run();
        }
    }

    println!("🎉 Build complete!");
    Ok(())
}

fn build_release(sh: &Shell, target: Option<String>) -> Result<()> {
    println!("🔨 Building release binaries...\n");

    let target_flag = match target {
        Some(t) => format!("--target {}", t),
        None => String::new(),
    };

    println!("📦 Building oxide-gui (release)...");
    cmd!(sh, "cargo build --release -p oxide-gui {target_flag}").run()?;

    println!("📦 Building oxide-service (release)...");
    cmd!(sh, "cargo build --release -p oxide-service {target_flag}").run()?;

    println!("📦 Building oxide-cli (release)...");
    cmd!(sh, "cargo build --release -p oxide-cli {target_flag}").run()?;

    println!("\n✅ Release binaries built successfully!");
    Ok(())
}

fn dev_gui(sh: &Shell) -> Result<()> {
    println!("🎨 Starting Oxide GUI (dev mode)...\n");
    cmd!(sh, "cargo run -p oxide-gui").run()?;
    Ok(())
}

fn dev_service(sh: &Shell) -> Result<()> {
    println!("⚙️  Starting Oxide Service (dev mode)...\n");
    cmd!(sh, "cargo run -p oxide-service").run()?;
    Ok(())
}

fn clean(sh: &Shell) -> Result<()> {
    println!("🧹 Cleaning build artifacts...");
    cmd!(sh, "cargo clean").run()?;
    println!("✅ Clean complete!");
    Ok(())
}

fn create_portable_readme(bundle_path: &str) -> Result<()> {
    let readme_content = r#"# Oxide Portable Release

## Quick Start

### Windows
Double-click `oxide.exe` to launch the Oxide GUI.

The service daemon will start automatically in the background.

### Linux/macOS
Open a terminal and run:
```bash
./oxide
```

Or make it executable first:
```bash
chmod +x oxide
./oxide
```

## Architecture

- **oxide** (GUI) - Visual interface for room management
- **oxide-service** (Daemon) - Persistent background service handling networking
- **assets/** - Resources (icons, etc)
- **config/** - Local configuration storage

## First Time Setup

1. Launch the GUI
2. Go to "Create Room" to host, or "Join Room" to join existing
3. Share the room code with other players
4. Click "Connect to Network" when all players are ready

## Troubleshooting

### GUI won't start
- Ensure you have graphics support on your system
- Check that your GPU drivers are up-to-date
- If running in WSL2, you may need to enable graphics support

### Can't connect to service
- Make sure oxide-service is running in background
- On first launch, the service may take a few seconds to initialize
- Check that port 8080 is available on localhost

### Network issues
- Ensure all machines can reach each other (firewalls/NAT)
- Run on the same local network or with proper VPN/port forwarding

## Support

For issues and questions, see the main repository README or documentation.

## License

GPL v3 - See LICENSE file in repository
"#;

    std::fs::write(format!("{}/README.txt", bundle_path), readme_content)?;
    Ok(())
}

fn create_windows_launcher(bundle_path: &str) -> Result<()> {
    let launcher_bat = format!(
        "@echo off\n\
setlocal enabledelayedexpansion\n\
\n\
REM Oxide Portable Launcher for Windows\n\
REM This script ensures the service runs and launches the GUI\n\
\n\
set SERVICE_PID_FILE=%TEMP%\\oxide-service.pid\n\
set SERVICE_EXE=%~dp0oxide-service.exe\n\
\n\
REM Start service if not already running\n\
if exist %SERVICE_PID_FILE% (\n\
    for /f %%i in (%SERVICE_PID_FILE%) do (\n\
        tasklist | find /i \"%%i\" >nul\n\
        if errorlevel 1 (\n\
            del %SERVICE_PID_FILE%\n\
            start /B %SERVICE_EXE%\n\
        )\n\
    )\n\
) else (\n\
    start /B %SERVICE_EXE%\n\
)\n\
\n\
REM Wait for service to be ready\n\
timeout /t 2 /nobreak\n\
\n\
REM Launch GUI\n\
start %~dp0oxide.exe\n\
\n\
exit /b 0\n\
"
    );

    std::fs::write(format!("{}/launch.bat", bundle_path), launcher_bat)?;
    Ok(())
}

fn create_unix_launcher(bundle_path: &str) -> Result<()> {
    let launcher_sh = "#!/bin/bash\n\n# Oxide Portable Launcher for Unix-like systems\n# This script ensures the service runs and launches the GUI\n\nSERVICE_PID_FILE=\"/tmp/oxide-service.pid\"\nSERVICE_EXE=\"$(dirname \"$0\")/oxide-service\"\nGUI_EXE=\"$(dirname \"$0\")/oxide\"\n\n# Start service if not already running\nif [ -f \"$SERVICE_PID_FILE\" ]; then\n    SERVICE_PID=$(cat \"$SERVICE_PID_FILE\")\n    if ! kill -0 \"$SERVICE_PID\" 2>/dev/null; then\n        rm \"$SERVICE_PID_FILE\"\n        \"$SERVICE_EXE\" &\n        echo $! > \"$SERVICE_PID_FILE\"\n    fi\nelse\n    \"$SERVICE_EXE\" &\n    echo $! > \"$SERVICE_PID_FILE\"\nfi\n\n# Wait for service to be ready\nsleep 2\n\n# Launch GUI\n\"$GUI_EXE\"\n\nexit 0\n";

    std::fs::write(format!("{}/launch.sh", bundle_path), launcher_sh)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(format!("{}/launch.sh", bundle_path), perms)?;
    }

    Ok(())
}
