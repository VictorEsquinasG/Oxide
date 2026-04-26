# 🤝 Contributing to the open VPN project

First off, thank you for considering contributing to this VPN! 🚀

This project aims to revive LAN multiplayer gaming for classic titles over the internet. To achieve this, we need to maintain a robust, secure, and cross-platform codebase.

Please read the following guidelines carefully to ensure a smooth collaboration process.

## ⚠️ Core Engineering Principles

To keep the project healthy, all contributions **must** adhere to these three golden rules:

### 1. 🧱 Respect Node Encapsulation
The architecture is strictly modular. We must maintain a clean separation of concerns:
* **`ui/`**: Only handles `egui` rendering and user input. It should **never** touch raw sockets or `tun` interfaces directly.
* **`network/`**: The `NetworkNode` handles the heavy lifting (UDP/TUN traffic). It should know nothing about the GUI.
* **`app/`**: Holds the shared state (`AppState`) acting as the bridge between UI and Network.
* **Do not break this isolation.** If you need to pass data, use channels or the shared `Arc<Mutex<AppState>>`.

### 2. 🖥️ 100% Cross-Platform Stability
This VPN must run on **Windows** and **Linux** simultaneously.
* **Do not introduce OS-specific breaking changes.**
* If you implement a feature that relies on a specific OS syscall (especially regarding the `tun` interface), you **must** use conditional compilation (`#[cfg(target_os = "linux")]`, etc.) and provide a fallback or a "no-op" for the other system.
* The code must compile and run on both platforms at all times.

### 3. 🌿 Branching & Pull Requests
* **Never push directly to `master`.**
* Create a new branch for every feature or fix.
* Submit a Pull Request (PR) only when the feature is **tested and complete**.

---

## 🗺️ Roadmap & Help Wanted

We are currently looking for contributors to help with:

1.  **🔒 Encryption (Priority):** Implementing `ChaCha20Poly1305` to encrypt the UDP payload.
2.  **🌐 Internationalization (i18n):** Extracting UI strings to allow English/Spanish switching.
3.  **🪟 Windows Driver Support:** Improving the `wintun` / `tap-windows` integration and setup experience.

---

## 🛠️ Development Workflow

### 1. Setup
1.  Fork the repository.
2.  Clone your fork:
    ```bash
    git clone [https://github.com/YOUR_USERNAME/Oxide.git](https://github.com/YOUR_USERNAME/Oxide.git)
    ```
3.  Ensure you have the necessary system dependencies (packet compilers on Linux, TAP drivers on Windows).

### 2. Create a Branch
Use a descriptive name for your branch:
```bash
git checkout -b feature/add-encryption
# or
git checkout -b fix/ui-alignment
