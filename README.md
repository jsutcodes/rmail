# 🦀 RMail

A blazing-fast, lightweight desktop and mobile email client built in **Rust**. This project acts as a custom client wrapper for Microsoft Outlook/365 work accounts, bringing native **Gmail-style workflows (like multi-labeling and "Send & Archive")** to the enterprise workspace using the Microsoft Graph API.

---

## 🔎 Gmail Features vs. Outlook (Architecture Mapping)

While Outlook is built around a traditional physical filing cabinet metaphor (one email lives in one folder), this app uses a database-tagging metaphor to achieve true Gmail functionality on top of an Outlook backend.

| Gmail Feature | How Outlook Compares / What It Lacks | How This App Solves It |
| :--- | :--- | :--- |
| **True Multi-Labeling** | Outlook uses Folders (one email per folder) or Categories. Archiving removes items from the Inbox but keeps them in a single folder. | **Core UI/UX:** Maps Outlook "Categories" to a custom multi-label UI view. |
| **Automatic Tabbed Sorting** | Outlook has a basic "Focused Inbox" (Focused vs. Other) but lacks multi-tab automatic sorting. | **Future Scope:** Can be custom-built using local filtering rules. |
| **"Send and Archive"** | Requires separate actions: clicking send, then manually moving the original thread to the Archive folder. | **Custom Macros:** One-click sends the reply and asynchronously moves the thread out of the Inbox. |
| **Nudges & Follow-ups** | Gmail automatically surfaces old emails at the top of the inbox with algorithmic reminders. | **Local Database Tracking:** Flags unanswered local cache entries after $X$ days. |
| **Advanced Search Chips** | Uses Google's search algorithms and intuitive UI filtering chips (e.g., `has:attachment`). | **Fast Local Search:** Powered by local SQLite indexing for instant filtering. |

---

## 🏗️ Technical Architecture ("Core + Shell")

To ensure high performance and code reuse across PC and Mobile, this project follows a strict **Core + Shell** separation:

*   **The Core (`rmail-core`):** A shared Rust library containing database schemas (`rusqlite`), Microsoft Graph API sync logic (`reqwest` + `serde`), OAuth state handling (`oauth2`), and state management.
*   **The Desktop Shell (`rmail-desktop`):** Powered by **Tauri**, utilizing the native OS webview for a lightweight frontend while running heavy sync logic natively in Rust.
*   **The Mobile Shell (`rmail-mobile`):** Uses **UniFFI** to generate native language bindings, allowing a native UI (SwiftUI for iOS, Jetpack Compose for Android) to call the compiled Rust core library directly.

---

## 🗺️ Roadmap & Development Blueprint

### 🖥️ Phase 1: PC Development (Desktop App)
*   [ ] **Step 1: Workspace Setup** — Initialize a Cargo workspace separating the core business logic from the Tauri desktop frontend.
*   [ ] **Step 2: Authentication & Security** — Integrate MSAL OAuth 2.0 flow via the `oauth2` crate. Capture token redirects via a temporary local server and securely store credentials using the `keyring` crate.
*   [ ] **Step 3: SQLite Cache Sync** — Implement local caching using `rusqlite` or `sqlx` to map and store emails, preserving multi-labeling states locally.
*   [ ] **Step 4: Mapping Outlook Categories** — Code the bridge that translates Outlook's string array `categories` property from the Microsoft Graph API into independent visual labels.
*   [ ] **Step 5: Send & Archive Implementation** — Build the UI and macro logic to send messages and concurrently clear them from the active Outlook Inbox folder.

### 📱 Phase 2: Phone App Development (Mobile App)
*   [ ] **Step 1: Foreign Function Interface (FFI)** — Define an interface definition file using `uniffi` to generate safe **Swift** (iOS) and **Kotlin** (Android) bindings for the core logic.
*   [ ] **Step 2: Cross-Compilation Pipeline** — Configure `cargo-ndk` and standard targets (`aarch64-apple-ios`) to compile the core Rust engine into native mobile libraries.
*   [ ] **Step 3: Native Mobile UI Shells** — Drop the generated libraries into Xcode and Android Studio to build high-performance mobile views.
*   [ ] **Step 4: Mobile Network Optimization** — Implement network retry wrappers with exponential backoff using `tokio-retry` to handle fluctuating cellular connections smoothly.

---

## 🛠️ Tech Stack Cheat Sheet

*   **Language:** Rust 🦀
*   **Desktop Wrapper:** Tauri
*   **Database:** SQLite (`rusqlite` / `sqlx`)
*   **HTTP Client:** `reqwest`
*   **Serialization:** `serde` + `serde_json`
*   **Auth:** `oauth2` + `keyring`
*   **Mobile Bindings:** `uniffi`

