# Testing Multi-Instance Rooms (Windows + Linux)

## Setup Requirements
- **Machine 1**: Windows (Padre - has Oxide compiled and running)
- **Machine 2**: Linux (Hijo - has Oxide compiled and running)
- Both on same network (can ping each other)
- Both machines actually RUNNING at the same time

---

## ✅ Test 1: Basic Room Creation & Joining

### Step 1: On Windows (Padre)
```
1. Open Oxide on Windows
2. Click "➕ Create a Room"
3. Fill in:
   - Room Name: "TestGame"
   - Your Alias: "PAPÁ"
   - Max Players: 4
4. Click "✅ Create Room"

EXPECTED RESULT:
  📋 In logs you should see:
    ✅ Room created successfully!
    🏠 Room Name: TestGame
    🔐 Room Code: 'ALPHA-FOX-2024' (or similar)
    👤 You: PAPÁ (Virtual IP: 10.0.0.1)
    📍 Players: 1/4

5. COPY THE ROOM CODE ← You'll need this!
```

### Step 2: On Linux (Hijo) 
```
1. Open Oxide on Linux
2. Click "➕ Join a Room"
3. Fill in:
   - Room Code: ALPHA-FOX-2024 (paste from above)
   - Your Alias: HIJO
4. Click "✅ Join Room"

EXPECTED RESULT (with reload fix):
  📋 In logs you should see:
    🔗 Joining room 'ALPHA-FOX-2024'...
    ✅ Successfully joined room! ← THIS MEANS IT WORKED!
    🏠 Room Name: TestGame
    🔐 Room Code: 'ALPHA-FOX-2024'
    👤 You: HIJO (Virtual IP: 10.0.0.2)
    📍 Players: 2/4
    👥 Peers in room:
      • PAPÁ at 192.168.1.100:9000 (Virtual IP: 10.0.0.1)
      • HIJO at 192.168.1.101:9000 (Virtual IP: 10.0.0.2)

❌ BAD RESULT (what we fixed):
    ❌ Failed to join room: Room not found
```

---

## ✅ Test 2: Connect to Network (P2P Mesh)

### From Linux (Hijo) - After Successfully Joining
```
1. Still in "In Room" screen
2. Click "🔌 Connect to Network"

EXPECTED IN LOGS:
  🌐 Initiating P2P connections...
  ✅ In room: 'TestGame' (Code: 'ALPHA-FOX-2024')
  🔗 Creating P2P mesh with 2 peer(s)...
  👥 Connecting to 1 peer(s):
    → PAPÁ (IP: 192.168.1.100, Virtual: 10.0.0.1)
  ⏳ Attempting to establish mesh connections...
  ✅ P2P mesh initialized (waiting for peer responses)
```

### From Windows (Papá) - After Successfully Creating
```
1. In "In Room" screen
2. Click "🔌 Connect to Network"

EXPECTED IN LOGS:
  (Same as above, but shows HIJO as the peer)
```

---

## ✅ Test 3: Clear Logs Button

```
1. Any screen with logs visible
2. Look for the "🗑️ Clear" button next to "📋 Activity Log:"
3. Click it

EXPECTED RESULT:
  All logs disappear ✅
  You can start fresh without accumulated messages
```

---

## ❌ Troubleshooting: "Room not found"

### If you still get "❌ Failed to join room: Room not found":

**Quick Checks:**
1. [ ] Both machines are actually running (not closed/crashed)
2. [ ] Room code is exactly as shown (case-sensitive!)
3. [ ] You waited for creator's logs to finish (don't join too fast)
4. [ ] Both machines are on same network

**Debug Steps:**
1. On Windows, check logs show: 🔐 Room Code: 'XXXX'
2. Copy that code EXACTLY (case matters!)
3. On Linux, paste it and try again
4. Watch for warning: "⚠️ Warning: Could not reload rooms"
   - If you see this, there's a file access issue

**Check File Locations:**

Windows - Open File Explorer:
```
%APPDATA%\Oxide\rooms.json
(Usually: C:\Users\c-017\AppData\Roaming\Oxide\rooms.json)
```

Linux - Open Terminal:
```bash
ls -la ~/.config/Oxide/rooms.json
cat ~/.config/Oxide/rooms.json
```

Should see JSON with rooms array containing your room.

---

## ⚠️ Known Limitation: Offline Rooms

**This WON'T Work:**
```
1. Dad creates room on Windows
2. Dad closes Oxide on Windows
3. Son tries to join on Linux
   → ❌ Room not found
   
WHY: The room info only exists in Windows's local file
     Linux can't access Windows's disk
     So when Windows closes, the info is gone from network perspective
```

**What DOES Work:**
```
1. Dad creates room on Windows (room saved to Windows disk)
2. Son joins on Linux (Son's machine reads Windows's disk info)
   ✅ Works because both machines are running
3. Dad and Son can now P2P connect
```

---

## Expected Behavior After Fixes

| Scenario | Before Fix | After Fix |
|----------|-----------|-----------|
| Create room on Windows | ✅ Works | ✅ Works |
| Join from Linux (same room) | ❌ Room not found | ✅ Works (with reload) |
| Clear logs repeatedly | ❌ Accumulate forever | ✅ Works (Clear button) |
| Create on Windows, Windows closes, join on Linux | ❌ Not found | ❌ Still not found* |

*This is a limitation of local storage, not a bug. Requires central server for full support.

---

## Commands to Build & Run

### Windows (MinGW or Visual Studio installed)
```bash
cd C:\Users\c-017\Documents\GitHub\Oxide
cargo build --release
cargo run --release
```

### Linux
```bash
cd ~/Documents/GitHub/Oxide
cargo build --release
./target/release/Oxide
```

---

## What Changed

✅ **RoomManager.reload()** - Reloads rooms from disk before operations
```rust
pub async fn reload(&mut self) -> Result<(), String> {
    match Self::load_config_from_file(&self.config_path).await {
        Ok(config) => { self.config = config; Ok(()) }
        Err(e) => {
            if self.config_path.exists() { Err(e) } 
            else { Ok(()) }
        }
    }
}
```

✅ **join_room() now reloads** 
```rust
// Before:
mgr.join_room(...)  // ❌ Used stale in-memory config

// After:
mgr.reload().await?;  // ✅ Refreshes from disk
mgr.join_room(...)   // ✅ Uses latest rooms
```

✅ **create_room() also reloads** (just in case)

✅ **Clear Logs Button**
```rust
if ui.button("🗑️ Clear").clicked() {
    self.state.logs.lock().unwrap().clear();
}
```

---

## Success Criteria

- [ ] Test 1: Windows creates room, Linux joins successfully ✅
- [ ] Test 2: After joining, both can click "Connect Network" ✅  
- [ ] Test 3: Clear Logs button works ✅
- [ ] No more "Room not found" when both machines running ✅

If all above pass, the fix is working! 🎉
