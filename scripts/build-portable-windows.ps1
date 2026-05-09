# Build Portable Release for Windows
# Usage: .\scripts\build-portable-windows.ps1 [-Release] [-Zip]

param(
    [switch]$Release,
    [switch]$Zip
)

$OutputDir = "dist"
$BundlePath = "$OutputDir\Oxide-Portable"

Write-Host "🔨 Building portable Oxide bundle..." -ForegroundColor Cyan

# Create output structure
if (-not (Test-Path $BundlePath)) {
    New-Item -ItemType Directory -Path $BundlePath\config | Out-Null
    New-Item -ItemType Directory -Path $BundlePath\assets | Out-Null
}

# Determine build profile
$ProfileFlag = if ($Release) { "--release" } else { "" }
$ProfileName = if ($Release) { "release" } else { "debug" }

Write-Host "`n📦 Building oxide-gui..." -ForegroundColor Yellow
& cargo build $ProfileFlag -p oxide-gui
if ($LASTEXITCODE -ne 0) { exit 1 }

Write-Host "`n📦 Building oxide-service..." -ForegroundColor Yellow
& cargo build $ProfileFlag -p oxide-service
if ($LASTEXITCODE -ne 0) { exit 1 }

# Copy binaries
Write-Host "`n📁 Copying binaries..." -ForegroundColor Yellow
Copy-Item "target\$ProfileName\oxide-gui.exe" "$BundlePath\oxide.exe"
Copy-Item "target\$ProfileName\oxide-service.exe" "$BundlePath\oxide-service.exe"

# Copy assets
Write-Host "📁 Copying assets..." -ForegroundColor Yellow
if (Test-Path "assets\Icon.png") {
    Copy-Item "assets\Icon.png" "$BundlePath\assets\"
}

# Create README
Write-Host "📝 Creating portable README..." -ForegroundColor Yellow

$ReadmeContent = @"
# Oxide Portable Release

## Quick Start

### Windows
Double-click `oxide.exe` to launch the Oxide GUI.

The service daemon will start automatically in the background.

### Troubleshooting

**GUI won't start:**
- Ensure you have graphics support on your system
- Check that your GPU drivers are up-to-date
- For WSL2/VirtualBox, graphics support may be limited

**Can't connect to service:**
- Wait a few seconds for oxide-service to initialize
- Check that port 8080 is available on localhost

**Network issues:**
- Ensure all machines can reach each other
- Check firewalls and NAT configurations

## Support

For issues, see the main repository README.

## License

GPL v3
"@

$ReadmeContent | Out-File -FilePath "$BundlePath\README.txt" -Encoding UTF8

# Create launcher batch file
$LauncherContent = @"
@echo off
setlocal enabledelayedexpansion

set SERVICE_EXE=%~dp0oxide-service.exe
set GUI_EXE=%~dp0oxide.exe

REM Start service in background
start /B %SERVICE_EXE%

REM Wait for service to initialize
timeout /t 2 /nobreak

REM Launch GUI
start %GUI_EXE%

exit /b 0
"@

$LauncherContent | Out-File -FilePath "$BundlePath\launch.bat" -Encoding ASCII

Write-Host "`n✅ Portable bundle created at: $BundlePath`n" -ForegroundColor Green

# Optional: Create ZIP archive
if ($Zip) {
    Write-Host "📦 Creating ZIP archive..." -ForegroundColor Yellow
    $ZipName = "$OutputDir\Oxide-Portable.zip"
    
    if (Get-Command Compress-Archive -ErrorAction SilentlyContinue) {
        Compress-Archive -Path $BundlePath -DestinationPath $ZipName -Force
        Write-Host "✅ ZIP archive created: $ZipName`n" -ForegroundColor Green
    } else {
        Write-Host "⚠️  Compress-Archive not available. Install PowerShell 5.0+ for ZIP support.`n" -ForegroundColor Yellow
    }
}

Write-Host "🎉 Build complete!" -ForegroundColor Green
