# 💡 Ejemplos de Uso Práctico

## Ejemplo 1: Creando tu primera sala (como HOST)

**Situación:** Quieres crear una sala para jugar en familia

```
PASO A PASO:

1. Abres Oxide
   → Ves pantalla principal con 3 botones

2. Haces click en "➕ Crear a Room"
   → Aparece pantalla CreateRoom

3. Completas el formulario:
   Room Name:    "Noche de Juegos Retro"
   Your Alias:   "Papá"
   Max Players:  6 (deslizas slider de 2 a 6)

4. Haces click en "✅ Create Room"
   → Sistema crea la sala y la guarda en JSON
   → Recibes código auto-generado

RESULTADO:
┌─────────────────────────────────────────┐
│ 🏠 Room: Noche de Juegos Retro         │
│ Code: Phantom-Knight-2027              │
│ Players: 1/6                           │
│                                         │
│ 👥 Players in Room:                     │
│ 🟢 Papá (10.0.0.1)                     │
│                                         │
│ [🔌 Connect Network] [📤 Leave Room]   │
└─────────────────────────────────────────┘

COMPARTIR CON FAMILIA:
📱 WhatsApp: "Hey, código: Phantom-Knight-2027"
```

---

## Ejemplo 2: Uniéndote a una sala (como GUEST)

**Situación:** Tu hijo quiere unirse a la sala que creaste

```
PASO A PASO:

1. Tu hijo abre Oxide en su PC
   → Ves pantalla principal

2. Hace click en "➕ Join a Room"
   → Aparece pantalla JoinRoom

3. Completa el formulario:
   Enter Room Code:  "Phantom-Knight-2027"
   Your Alias:       "Hijo"

4. Hace click en "✅ Join Room"
   → Sistema busca la sala en JSON
   → Te agrega como nuevo peer
   → Te asigna IP virtual: 10.0.0.2

RESULTADO:
┌─────────────────────────────────────────┐
│ 🏠 Room: Noche de Juegos Retro         │
│ Code: Phantom-Knight-2027              │
│ Players: 2/6                           │
│                                         │
│ 👥 Players in Room:                     │
│ 🟢 Papá (10.0.0.1)                     │
│ 🟢 Hijo (10.0.0.2)                     │
│                                         │
│ [🔌 Connect Network] [📤 Leave Room]   │
└─────────────────────────────────────────┘

EN LA PANTALLA DEL PAPÁ (se actualiza automáticamente):
┌─────────────────────────────────────────┐
│ 🏠 Room: Noche de Juegos Retro         │
│ Code: Phantom-Knight-2027              │
│ Players: 2/6  ← CAMBIÓ DE 1/6          │
│                                         │
│ 👥 Players in Room:                     │
│ 🟢 Papá (10.0.0.1)                     │
│ 🟢 Hijo (10.0.0.2)  ← NUEVO            │
│                                         │
│ [🔌 Connect Network] [📤 Leave Room]   │
└─────────────────────────────────────────┘
```

---

## Ejemplo 3: Sala completa con múltiples personas

**Situación:** 3 miembros de la familia se unen a la sala

```
ESTADO FINAL EN TODOS LOS DISPOSITIVOS:
┌──────────────────────────────────────────────┐
│ 🏠 Room: Noche de Juegos Retro             │
│ Code: Phantom-Knight-2027                  │
│ Players: 3/6                               │
│                                            │
│ 👥 Players in Room:                        │
│ 🟢 Papá (10.0.0.1)                        │
│ 🟢 Hijo (10.0.0.2)                        │
│ 🟢 Hija (10.0.0.3)                        │
│                                            │
│ [🔌 Connect Network] [📤 Leave Room]      │
└──────────────────────────────────────────────┘

ESTADO DE CONEXIÓN:
Papá:   🟢 Online   (Host)
Hijo:   🟢 Online   (Conectado)
Hija:   🟢 Online   (Conectada)

PRÓXIMO PASO (Phase 3):
Click "🔌 Connect Network" → P2P mesh se establece
Entonces pueden ejecutar juegos LAN antiguos
y compartir conexión de red virtual
```

---

## Ejemplo 4: Archivo JSON generado (guardado automático)

**Ubicación:** `C:\Users\[usuario]\AppData\Roaming\Oxide\rooms.json`

```json
{
  "version": 1,
  "default_alias": "Player",
  "rooms": {
    "Phantom-Knight-2027": {
      "id": "Phantom-Knight-2027",
      "name": "Noche de Juegos Retro",
      "creator_id": "papá_pc",
      "virtual_network": "10.0.0.0/24",
      "peers": {
        "papá_pc": {
          "id": "papá_pc",
          "alias": "Papá",
          "virtual_ip": "10.0.0.1",
          "real_ip": "203.0.113.50",
          "port": 9000,
          "status": "Online",
          "last_seen": 1712500235
        },
        "hijo_pc": {
          "id": "hijo_pc",
          "alias": "Hijo",
          "virtual_ip": "10.0.0.2",
          "real_ip": "198.51.100.42",
          "port": 9000,
          "status": "Online",
          "last_seen": 1712500230
        },
        "hija_pc": {
          "id": "hija_pc",
          "alias": "Hija",
          "virtual_ip": "10.0.0.3",
          "real_ip": "192.0.2.88",
          "port": 9001,
          "status": "Connecting",
          "last_seen": 1712500220
        }
      },
      "max_players": 6,
      "active": true,
      "created_at": 1712500000,
      "last_activity": 1712500235
    },
    "Alpha-Fox-2025": {
      "id": "Alpha-Fox-2025",
      "name": "Sala de Prueba",
      "creator_id": "test_user",
      "virtual_network": "10.0.0.0/24",
      "peers": {
        "test_user": {
          "id": "test_user",
          "alias": "TestPlayer",
          "virtual_ip": "10.0.0.1",
          "real_ip": "192.0.2.1",
          "port": 9000,
          "status": "Offline",
          "last_seen": 1712400000
        }
      },
      "max_players": 4,
      "active": false,
      "created_at": 1712400000,
      "last_activity": 1712400050
    }
  }
}
```

**Nota:** El archivo se actualiza automáticamente cada vez que:
- Creas una sala
- Te unes a una sala
- Cambias de estado
- Salís de una sala

---

## Ejemplo 5: Navegación entre pantallas

```
FLUJO COMPLETO DE USUARIO:

┌─────────────────────┐
│   App Arranca       │
└──────────┬──────────┘
           │
           ↓
┌─────────────────────────────────────┐
│  MainMenu (Pantalla inicial)         │
│                                      │
│  1. ➕ Create a Room                 │
│  2. ➕ Join a Room                   │
│  3. ⚙️  Legacy Mode (Direct P2P)    │
│  4. ❌ Exit                          │
└─────────────────────────────────────┘
     │         │         │
     │         │         └──────────┐
     ↓         ↓                     ↓
   Create    Join               Legacy
   Room      Room              P2P Mode
     │         │                    │
     ↓         ↓                    ↓
 ┌──────┐  ┌──────┐           ┌────────┐
 │Enter │  │Enter │           │Enter   │
 │Room  │  │Room  │           │Peer IP │
 │Name  │  │Code  │           │&Port   │
 │&Alias│  │&Alias│           │        │
 └──────┘  └──────┘           └────────┘
     │         │                    │
     ↓         ↓                    ↓
 [Create]  [Join]            [Connect]
     │         │                    │
     │         │                    │
     └─────┬───┴────────────────────┘
           │
           ↓
    ┌─────────────────┐
    │  In Room        │
    │  (Final State)  │
    └─────────────────┘
           │
           ├──→ [Connect Network]  (Phase 3)
           │
           └──→ [Leave Room] → MainMenu
```

---

## Ejemplo 6: Escenario Real - 4 Computadoras diferentes

**Casa:** Papá quiere jugar Diablo con sus hijos (remoto)

```
PC 1 - PAPÁ (Host)
  IP: 203.0.113.50
  SO: Windows
  
  Abre Oxide → Crear Sala
  Nombre: "Diablo Party"
  Alias: "Papá"
  Max Players: 4
  
  → Código generado: "Epic-Demon-2027"
  → Su IP virtual: 10.0.0.1
  
─────────────────────────────────────────

PC 2 - HIJO (Madrid)
  IP: 198.51.100.42
  SO: Windows
  
  Abre Oxide → Unirse a Sala
  Código: "Epic-Demon-2027"
  Alias: "Hijo1"
  
  → Asignado: 10.0.0.2
  → Estado: 🟢 Online
  
─────────────────────────────────────────

PC 3 - HIJA (Barcelona)
  IP: 192.0.2.88
  SO: Linux
  
  Abre Oxide → Unirse a Sala
  Código: "Epic-Demon-2027"
  Alias: "Hija"
  
  → Asignado: 10.0.0.3
  → Estado: 🟢 Online
  
─────────────────────────────────────────

PC 4 - SOBRINO (Valencia)
  IP: 192.0.2.200
  SO: macOS
  
  Abre Oxide → Unirse a Sala
  Código: "Epic-Demon-2027"
  Alias: "Sobrino"
  
  → Asignado: 10.0.0.4
  → Estado: 🟢 Online

─────────────────────────────────────────

RESULTADO EN TODOS:
┌──────────────────────────────────────┐
│ 🏠 Room: Diablo Party               │
│ Code: Epic-Demon-2027               │
│ Players: 4/4 [FULL]                 │
│                                      │
│ 👥 Players in Room:                  │
│ 🟢 Papá (10.0.0.1)    - Windows      │
│ 🟢 Hijo1 (10.0.0.2)   - Windows      │
│ 🟢 Hija (10.0.0.3)    - Linux        │
│ 🟢 Sobrino (10.0.0.4) - macOS        │
│                                      │
│ [🔌 Connect Network] [📤 Leave]     │
└──────────────────────────────────────┘

Ahora (Phase 3) harán click:
[🔌 Connect Network]
↓
Se establece mesh P2P entre 4 máquinas
↓
Ejecutan Diablo
↓
Diablo ve IPs virtuales 10.0.0.1-4
↓
LAN multiplayer ¡FUNCIONA REMOTAMENTE!
```

---

## Ejemplo 7: Manejo de errores común

```
ESCENARIO 1: Código de sala incorrecto

Usuario entra: "Alpha-Fox-INVALID"
Sistema busca en rooms.json
→ No encontrado
→ Error: "❌ Room not found"
→ Mostrar en log de actividad


ESCENARIO 2: Sala llena

5 personas en sala (max 5)
Intenta 6ª persona unirse
Sistema verifica: room.peers.len() >= max_players
→ Error: "❌ Room is full"
→ Debe crear sala nueva o esperar a que alguien salga


ESCENARIO 3: Alias vacío

Usuario intenta unirse sin escribir alias
Sistema verifica: self.ui.player_alias.is_empty()
→ Error: "❌ Player alias cannot be empty"
→ Debe escribir un nombre


ESCENARIO 4: Nombre de sala vacío

Usuario intenta crear sala sin nombre
Sistema verifica: self.ui.room_name.is_empty()
→ Error: "❌ Room name cannot be empty"
→ Debe completar nombre
```

---

## Ejemplo 8: Ciclo de vida completo de una sesión

```
TIMELINE - Noche de juegos en familia:

18:00 - CREACIÓN
Papá abre Oxide
→ Crea sala "Noche de Viernes"
→ Código: "Swift-Kraken-2027"
→ rooms.json creado/actualizado

18:05 - PRIMERAS CONEXIONES
Hijo abre Oxide
→ Se une con código
→ Ahora: 2 personas
Hija abre Oxide
→ Se une con código
→ Ahora: 3 personas

18:10 - SALA LISTA
Todos ven:
🟢 Papá (10.0.0.1)
🟢 Hijo (10.0.0.2)
🟢 Hija (10.0.0.3)
→ Click [Conectar Red]
→ Mesh P2P se establece

18:15 - JUEGO
Todos ejecutan Diablo (juego LAN)
Diablo se conecta a 10.0.0.1-3
¡Multiplayer funciona!

20:00 - DESCONEXIÓN
Hijo sale primero
→ rooms.json: Hijo offline
Papá y Hija siguen jugando

20:30 - FIN DE SESIÓN
Papá sale
→ rooms.json: solo Hija
Hija sale
→ rooms.json: sala vacía
→ rooms.json: active = false

23:00 - REINICIO SEMANA SIGUIENTE
Papá vuelve a abrir Oxide
→ Carga rooms.json
→ Ve sala anterior
→ Click [Unirse]
→ ¡Misma sala, mismo código!
→ Puede reutilizarla indefinidamente
```

---

## Resumen Técnico

| Concepto | Ejemplo |
|----------|---------|
| **Room Code** | `Phantom-Knight-2027` |
| **Virtual IP Range** | `10.0.0.1` - `10.0.0.254` |
| **Max Players** | 10 (configurable 2-10) |
| **Persistence** | JSON file on disk |
| **Aliases** | "Papá", "Hijo", "Hija" |
| **Status** | 🟢 Online, 🔴 Offline, 🟡 Connecting |
| **Plataformas** | Windows, macOS, Linux |

---

¡Así es como funcionará tu aplicación! 🎮
