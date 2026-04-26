# Script para descargar e instalar Npcap silenciosamente
# Ejecutar como: powershell -ExecutionPolicy Bypass -File install-npcap.ps1

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "      Instalador de Npcap para Oxide" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# Verificar si Npcap ya está instalado
$npcapDirs = @("C:\Npcap", "C:\Program Files\Npcap")
$installed = $false

foreach ($dir in $npcapDirs) {
    if (Test-Path "$dir\Lib\x64\Packet.lib") {
        Write-Host "[OK] Npcap ya esta instalado en: $dir" -ForegroundColor Green
        $installed = $true
        break
    }
}

if ($installed) {
    Write-Host ""
    Write-Host "La compilacion deberia funcionar ahora. Ejecuta:" -ForegroundColor Yellow
    Write-Host "cargo build --release" -ForegroundColor Cyan
    exit 0
}

# Descargar Npcap
$tempDir = "$env:TEMP\npcap_setup"
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

$installerPath = "$tempDir\npcap-1.13.exe"
$npcapUrl = "https://nmap.org/npcap/dist/npcap-1.13.exe"

Write-Host "[*] Descargando Npcap..." -ForegroundColor Yellow
try {
    Invoke-WebRequest -Uri $npcapUrl -OutFile $installerPath -UseBasicParsing -ErrorAction Stop
    Write-Host "[OK] Descarga completada" -ForegroundColor Green
} catch {
    Write-Host "[ERROR] Error descargando Npcap: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Descarga manual desde: https://nmap.org/npcap/" -ForegroundColor Yellow
    exit 1
}

# Instalar Npcap
Write-Host ""
Write-Host "[*] Instalando Npcap..." -ForegroundColor Yellow
Write-Host "    (Este proceso puede tomar 1-2 minutos)" -ForegroundColor Gray

try {
    & $installerPath /S /D="C:\Npcap" 2>&1 | Out-Null
    Start-Sleep -Seconds 2
    Write-Host "[OK] Instalacion completada" -ForegroundColor Green
} catch {
    Write-Host "[ERROR] Error instalando Npcap: $_" -ForegroundColor Red
    exit 1
}

# Verificar instalación
Write-Host ""
Write-Host "[*] Verificando instalacion..." -ForegroundColor Yellow

if (Test-Path "C:\Npcap\Lib\x64\Packet.lib") {
    Write-Host "[OK] Npcap instalado correctamente" -ForegroundColor Green
    Write-Host ""
    Write-Host "================================================" -ForegroundColor Green
    Write-Host "[OK] INSTALACION EXITOSA" -ForegroundColor Green
    Write-Host "     Npcap esta listo para compilar" -ForegroundColor Green
    Write-Host "================================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Ahora ejecuta: cargo build --release" -ForegroundColor Cyan
} else {
    Write-Host "[!] Advertencia: Npcap se instalo pero no se encontro Packet.lib" -ForegroundColor Yellow
    Write-Host "    Por favor, reinstala Npcap manualmente" -ForegroundColor Yellow
}

# Limpiar
Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
