# Guía de Instalación de Wintun para HecateVPN

## Descripción General

HecateVPN utiliza **Wintun**, un driver de red en espacio de usuario para Windows, para crear adaptadores de red virtuales. Este documento explica la configuración y la solución de problemas.

## ¿Qué es Wintun?

Wintun es un driver de red de código abierto y libre para Windows que permite a las aplicaciones crear y gestionar interfaces de red virtuales sin requerir drivers en modo kernel o configuración compleja del sistema.

## Instalación Automática (Recomendado)

HecateVPN incluye detección e instalación automática de Wintun:

### Primera ejecución:
1. Al iniciar la app, se verifica automáticamente si Wintun está instalado
2. Si **no** está instalado, aparecerá un diálogo cuando intentes conectarte
3. Acepta la instalación y la app descargará e instalará Wintun 0.14.1 automáticamente
4. **Verás en los logs el progreso de instalación** (descarga, instalación, etc.)
5. Una vez completada, aparecerá un mensaje pidiendo que **reinicies HecateVPN**
6. Al reiniciar, Wintun estará disponible y podrás conectarte

### Comportamiento de instalación:
- ✅ La instalación ocurre en **segundo plano** (no bloquea la UI)
- ✅ El progreso se muestra en tiempo real en los logs
- ✅ Si falla, reintentar automáticamente con permisos elevados
- ✅ Se mostrará un diálogo con el resultado (éxito o error)
- ✅ **La app NO se cierra automáticamente** - puedes cerrarla cuando quieras

## Instalación Manual

Si la instalación automática falla, puedes instalar Wintun manualmente:

### Opción 1: Descarga y Instalación Directa

1. Descarga desde: https://www.wintun.net/
2. Ejecuta el instalador: `wintun-0.14.1-amd64.msi`
3. Sigue el asistente de instalación
4. Reinicia HecateVPN

### Opción 2: Instalación por Línea de Comandos (PowerShell como Administrador)

```powershell
# Descargar
Invoke-WebRequest -Uri "https://www.wintun.net/builds/wintun-0.14.1-amd64.msi" -OutFile "$env:TEMP\wintun.msi"

# Instalar
msiexec /i "$env:TEMP\wintun.msi" /quiet /norestart

# Limpiar
Remove-Item "$env:TEMP\wintun.msi"
```

## Solución de Problemas

### Error: "LoadLibraryExW failed"

Significa que `wintun.dll` no se encuentra. Soluciones:

1. **Verificar instalación**:
   ```powershell
   Test-Path "C:\Windows\System32\wintun.dll"
   ```

2. **Reinstalar Wintun**:
   - Panel de Control → Programas → Programas y características → Busca "Wintun" y desinstala
   - Reinicia tu computadora
   - Usa el instalador automático de HecateVPN o instala manualmente

3. **Verificar versión de Windows**:
   - Wintun requiere Windows 7 SP1 o superior
   - Verifica con: `[System.Environment]::OSVersion.VersionString`

### Error: "Instalación fallida" o "No se pueden obtener permisos elevados"

**La app debe ejecutarse como Administrador para instalar Wintun:**

1. Haz clic derecho en `HecateVPN.exe`
2. Selecciona "Ejecutar como administrador"
3. Intenta instalar nuevamente

Si aún falla:
- Desactiva temporalmente el antivirus o software de seguridad
- Intenta la instalación manual desde PowerShell como Administrador
- Revisa el evento en registros de eventos de Windows

### Wintun está instalado pero la app no lo detecta

1. Reinicia completamente HecateVPN
2. Verifica que `wintun.dll` esté en el lugar correcto:
   ```powershell
   Get-ChildItem -Path "C:\Windows\System32" -Filter "wintun.dll"
   ```
3. Si no está, reinstala usando el método anterior

## Cambios Recientes (v2.0)

### Mejoras en la Instalación:

✅ **Instalación No Bloqueante**
- La instalación ahora ocurre en un background task
- La UI no se congela durante la descarga/instalación

✅ **Progreso en Tiempo Real**
- Todos los pasos se muestran en los logs de la app
- 📦 Descargando Wintun...
- ✅ Descargado (X bytes)
- 📋 Ejecutando instalador MSI...
- ⚠️ Reintentando con permisos elevados (si es necesario)
- ✅ Instalación completada. Por favor reinicia HecateVPN.

✅ **Mejor Manejo de Errores**
- Si falla la primera vez, reintentar automáticamente con permisos elevados
- Mensajes de error claros y detallados
- No cierra la app automáticamente después de instalar

✅ **Sin Cierre Automático**
- La app permanece abierta después de la instalación
- El usuario decide cuándo reiniciar la app
- Permite revisar los logs de instalación

## Detalles Técnicos

- **Versión de Wintun**: 0.14.1
- **URL de descarga**: https://www.wintun.net/builds/wintun-0.14.1-amd64.msi
- **Método de instalación**: Instalador MSI vía `msiexec`
- **Ubicación de instalación**: `C:\Windows\System32\wintun.dll`
- **Carga de biblioteca**: Usa la crate `libloading` para cargar dinámicamente `wintun.dll`
- **Runtime asincrónico**: Tokio para operaciones sin bloqueo

## Archivos Implementados

- [src/system/wintun.rs](src/system/wintun.rs) - Lógica de instalación con callbacks de progreso
- [src/ui/egui_ui.rs](src/ui/egui_ui.rs) - Integración en la UI con manejo del estado
- [src/main.rs](src/main.rs) - Verificación en startup

## Notas de Seguridad

- La instalación de Wintun requiere permisos de administrador
- El MSI se descarga del sitio web oficial de Wintun
- Ningún tráfico de red es modificado por Wintun; solo proporciona la interfaz del adaptador
- HecateVPN controla todo el tráfico que fluye a través del adaptador virtual

## Soporte

Si encuentras problemas:

1. Revisa esta guía (sección Solución de Problemas)
2. Verifica manualmente la instalación de Wintun
3. Intenta reinstalar Wintun
4. Reinicia Windows si es necesario
5. Contacta con soporte de HecateVPN con mensajes de error y versión de Windows
