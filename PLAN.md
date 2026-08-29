# yt-shortmaker v2 — Project Plan (Living Document)

> **Status:** Active Development (M0-M3 shipped, config + pipeline in progress)
> **Stack:** Rust 2021, **Slint 1.17** (native, no webview), `tokio`, workspace `core` + `app` + `cli`
> **Legacy:** v1 (`src/main.rs` ratatui/crossterm + egui experiment) purged — fresh start, no backwards compatibility
> **AI:** Extensible `AiProvider` trait — `GeminiProvider` is the first provider (native video)
> **Date:** 2026-08-29
> **UI rules:** See `STYLE.md` — mandatory visual and interaction constraints

---

## 1. Vision & Non-Goals

**Vision:** A lightweight desktop app for Windows & Linux that downloads a YouTube video, splits it into chunks, analyzes each chunk with AI for best moments, lets the user review/edit moments, and exports vertical 9:16 shorts via a declarative `Plano` compositing system — native GUI (Slint), no webview.

**Non-Goals:**

- No webview, no Tauri/Electron/HTML.
- No retro-compatibility with v1 `settings.json` — fresh config at `config_dir/yt-shortmaker-v2/`.
- No non-native-video AI providers at launch — trait is ready, Gemini only.

---

## 2. GUI Decision: Slint

The v2 app uses **Slint 1.17** (`crates/app/ui/app.slint`) with a fixed dark shell (SidePanel nav + content router). The visual language is intentionally flat and square: no rounded cards, no light/dark toggle, and no generic dashboard styling. egui was evaluated in M0 POC and replaced by Slint for a more native look and a declarative UI. `core` remains UI-agnostic (no Slint dependency), so a future UI migration is possible without touching business logic.

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────┐
│  crates/app  (Slint 1.17)               │
│  - ui/app.slint (MainWindow, router)    │
│  - main.rs callbacks -> core (sync)     │
└────────────────┬────────────────────────┘
                 │ calls core
┌────────────────▼────────────────────────┐
│  crates/core  (lib, no UI deps)         │
│  config | media | ai | plano | session  │
│  security | setup | types               │
└────────────────┬────────────────────────┘
                 │ spawns
┌────────────────▼────────────────────────┐
│  ffmpeg / yt-dlp (child processes)      │
│  Gemini File API + generateContent      │
└─────────────────────────────────────────┘
```

- **Pattern:** core exposes pure functions (sync + async). The app keeps a `Rc<RefCell<AppConfig>>` and Slint in-out properties; long operations run via tokio with `CancellationToken` (`tokio_util`) and progress forwarded through callbacks / channels.
- **Cancellation:** `tokio_util::CancellationToken`.
- **Error:** `anyhow` in `core`, surfaced in UI as status messages / error text.

---

## 4. Workspace Layout

```text
yt-shortmaker/
  Cargo.toml              # [workspace] members = ["crates/core","crates/app","crates/cli"]
  Cargo.lock
  rust-toolchain.toml     # channel = "stable"
  deny.toml / clippy.toml
  locales/{en,es,ru}.yml  # rust-i18n 4.2
  PLAN.md                 # this file
  STYLE.md                # mandatory UI and project style rules
  docs/
  crates/
    core/                 # UI-agnostic business logic
      src/
        lib.rs
        config/{mod.rs,store.rs}
        types/mod.rs
        media/{mod.rs,ffmpeg.rs,ytdlp.rs,pipeline.rs,chunk.rs}
        ai/{mod.rs,provider.rs,gemini.rs}
        plano/{mod.rs,schema.rs,compiler.rs,preview.rs}
        session/mod.rs
        security/{mod.rs,keyring.rs}
        setup/mod.rs
    app/                  # Slint GUI
      ui/app.slint
      src/main.rs
    cli/                  # clap 4
      src/main.rs
```

---

## 5. AI Provider Handler (Extensible)

```rust
// crates/core/src/ai/provider.rs
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn analyze_chunk(&self, ctx: AnalyzeCtx) -> anyhow::Result<Vec<VideoMoment>>;
    async fn test_connection(&self) -> anyhow::Result<()>;
}
```

- `ProviderRegistry` holds `HashMap<String, Arc<dyn AiProvider>>` + `active_id`.
- `GeminiProvider` is the reference implementation (native video: File API upload → `wait_for_file_active` → `generateContent` with structured output).
- Future providers plug in via the trait; if `supports_native_video == false`, the pipeline errors with guidance.

---

## 6. Core Subsystems

### 6.1 Config (v2, no v1 compat)

Path: `dirs::config_dir()/yt-shortmaker-v2/settings.json`. `store.rs` does load/save with serde defaults (old files load with new fields defaulted), atomic write via tmp + rename, and range validation.

Schema (extended 2026-08-29):

```json
{
  "version": 2,
  "language": "en",
  "output_dir": null,
  "cookies": { "use_cookies": false, "path": "" },
  "processing": {
    "chunk_size_secs": 600,
    "max_last_chunk_secs": 900,
    "chunk_delay_secs": 2,
    "retry_attempts": 3,
    "auto_extract": false
  },
  "export": { "naming_template": "short_%Y%m%d_%H%M%S", "ffmpeg_path": null },
  "ai": {
    "active_provider": "gemini",
    "providers": {
      "gemini": {
        "keys": [],
        "model": "gemini-2.5-flash",
        "model_pro": "gemini-2.5-pro",
        "use_fast_model": true,
        "temperature": null,
        "media_resolution": "720P"
      }
    }
  }
}
```

- Chunk size is edited in **minutes** in the UI, stored in **seconds** (`chunk_size_minutes()` / `set_chunk_size_minutes()`).
- Keys are stored as `keyring:` references via `security::keyring`; the raw value lives in the OS credential store.

### 6.2 Types

`VideoMoment`, `DialoguePhrase`, `VideoChunk` (+ `HH:MM:SS` parse/format helpers) ported from v1 with tests. `SessionState`/`APP_VERSION` removed as dead v1 residue.

### 6.3 Media

- **ffmpeg.rs:** `get_duration`, `get_resolution` (now logged in pipeline). `get_duration_u64` removed as trivial wrapper. Centralized availability via `setup::dependency_status`.
- **chunk.rs:** `calculate_chunks(total_secs, chunk_size_secs, max_last_chunk_secs) -> Vec<(u64,u64)>` (start, duration) — ported from v1, now config-driven (v1: 10 min chunks, merge last ≤15 min). `split_video(...)` async: ffmpeg `-ss`/`-t` per chunk with per-chunk cancellation and rich stderr.
- **ytdlp.rs:** `extract_video_id`, `validate_media_url`, `fetch_info` and `download_video` (low/high res, cookies, retry + NotFound mapping to per-OS install guidance). No local `is_*` check — uses `setup`.
- **pipeline.rs:** `Stage` state machine (Downloading → Splitting → Analyzing → Extracting) + `Pipeline::run` with progress callbacks, cancellation, DB persistence, `setup::check_dependencies` fail-fast, resolution log via `get_resolution`, and provider progress channel (`ProviderEvent` → `PipelineEvent`).

### 6.4 Plano

`PlanoObject::{Clip, Image, Shader { Blur }, Video}` schema, `build_ffmpeg_filter` compiler, default 3-layer plano (blurred bg + Shader Blur + centered Clip). `load_plano`/`create_default_plano`/`save_plano` used by CLI and pipeline auto-extract placeholder; `preview_info` wired to pipeline log. Replaces v1 `ShortsConfig` (blur/zoom/height/opacity now live in the Plano JSON).

### 6.5 Session (SQLite)

`rusqlite 0.40` (bundled) at `data_local_dir()/yt-shortmaker-v2/session.db`. Tables: `projects`, `chunks`, `moments`, `exports`. Replaces v1 `temp.json` + `cache_{id}`. `init_db` + `db_path` implemented; CRUD helpers (`list_projects` wired to Home, `get_project`/`get_moments` used in Home refresh, `delete_project` ready for M4) wired into pipeline + UI.

### 6.6 Security

Modes: `None` (dev) / `Keyring` (default, `keyring 3.6`). `Password` (`aes-gcm`+`argon2`) removed — deps pruned until implemented. `Secret` zeroize removed with it. `keyring::set_secret`/`get_secret`/`delete_secret` all wired in Keys page (Add/Delete/Toggle/Test with error handling, no `let _ =`).

### 6.7 Setup

`dependency_status()` + `check_dependencies()` + `format_missing_message()` with per-OS guidance, used as single source of truth by pipeline and `app` startup banner (`update_deps_ui`). Wires `ffmpeg`/`ffprobe`/`yt-dlp`. Auto-download (`ensure_tools`) still planned — not yet implemented (criterion “one-command install” pending).

---

## 7. App (Slint) UX Flow

```
SidePanel: Home | New Project | Export Standalone | Settings | Keys
Home: recent projects (DB) + empty state
New Project: URL input + Fetch Info (validate) + Start Analyze (progress bar, status)
Settings: output dir (Browse), language, chunk size (minutes), auto-extract,
          cookies (bool + path), naming template
AI / Keys: provider model selection + API key entry (display name generated automatically)
Export: standalone export placeholder
```

Settings are persisted via `store::save`; translations applied via `apply_translations` (`tr_*` Slint properties from `locales/*.yml`).

---

## 8. CLI

`crates/cli` with `clap 4.6`: `preview`, `transform`, `batch`, `analyze` (headless, currently mock output — real pipeline planned). Shares `core` 100%.

---

## 9. Testing Strategy

- **Unit (core):** config serde defaults + validation, `calculate_chunks` boundary cases, timestamp helpers, plano filter snapshots, gemini JSON parsing.
- **Integration:** real ffmpeg with tiny fixtures; Gemini via mock.
- **UI:** manual QA + Slint runtime smoke test.
- Gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.

---

## 10. CI/CD & Packaging

`.github/workflows` with release matrix (Windows exe + MSI via `wix/`, Linux binary + deb via `cargo-deb`, aarch64 via `cross`). `crates/app/build.rs` embeds the icon via `winresource`. `Cross.toml` for cross builds. Version from workspace `Cargo.toml` (2.0.0).

---

## 11. i18n, Logging, Error Handling

- **i18n:** `rust-i18n 4.2` + `locales/{en,es,ru}.yml`; the Slint UI receives translated strings via `tr_*` properties set from `t!()`.
- **Logging:** `tracing` + `tracing-subscriber` (env-filter); app and CLI init subscribers at startup.
- **Error:** `anyhow` in core; surfaced as status/error text in Slint.

---

## 12. Milestones — Status

| Milestone | Scope | Status |
|---|---|---|
| M0 Bootstrap | Workspace, Slint shell, CI | ✅ Shipped |
| M1 Core crate | config/types/media/ai/plano/session/security/setup | ✅ Shipped (dead code pruned: `SessionState`/`APP_VERSION`, `aes-gcm`/`argon2`/`Secret`; `get_resolution` wired) |
| M2 Media pipeline | downloads, chunking, split, pipeline run | ✅ Shipped (real `fetch_info`/`download_video`/`Pipeline::run` with retry, `check_dependencies` fail-fast, `ProviderEvent` channel) |
| M3 App shell | Slint nav, Home, Settings, Keys, i18n | ✅ Shipped (Home `list_projects` wired, Keys `get/delete/toggle` + `test_connection` wired, `windows_subsystem` fix, deps banner + re-check) |
| M4 Analyze & Review | Ingest, Analyze page, Review moments table | 🔄 In progress (Ingest/Analyze done + persisted; `get_moments`/`get_project` readable, Review table pending) |
| M5 Export & Plano editor | Plano editor, preview, batch export | ⏳ Pending (`save_plano`/`preview_info` wired to auto_extract placeholder, full editor pending) |
| M6 CLI & Polish | real `analyze`, shortcuts, first-run onboarding | 🔄 In progress (`analyze` runs real pipeline, `preview`/`transform`/`batch` still mock `filter` only; onboarding = deps banner) |
| M7 Packaging & QA | release matrix, installers, E2E | ⏳ Pending |
| M8 Beta release | v2.0.0 tag + assets | ⏳ Pending |

---

## 13. Acceptance Criteria for v2.0.0

- All milestones DoD met; `cargo test` + `cargo clippy -- -D warnings` clean on Windows + Linux.
- No `ratatui`/`crossterm`/`egui` code remains; no `Simple` encryption; no `temp.json`.
- User journey `URL → Analyze (Gemini) → Review (edit) → Export (Plano) → Shorts` works end-to-end with a real Gemini key.
- One-command install for 90% of users (auto ffmpeg/yt-dlp).

---

## 14. Roadmap Notes / Decisions Log

- 2026-08-29: Reverted `PLAN.md` stack note — the shipped GUI is **Slint 1.17**, not egui (egui was an M0 POC, dropped).
- 2026-08-29: Config schema extended with `cookies`, `processing`, `export` and per-provider `model_pro`/`use_fast_model`/`temperature`/`media_resolution`, restoring v1 capabilities (chunk timing, processing behavior, cookies, auto-extract) in a v2-native shape.
- 2026-08-29: Model selection belongs to the global **AI / Keys** area, not the New Project process. API key entry only asks for the secret; display names are generated by the app.
- 2026-08-29: The UI keeps one dark, flat visual language. No light/dark setting is exposed or persisted.
- v1 `ShortsConfig` deliberately not ported — its fields (blur, zoom, main height, bg opacity, overlays) live in the Plano JSON.
