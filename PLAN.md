# yt-shortmaker v2 — Migration Plan (Complete Rewrite)

> **Status:** Planning / Approved for Build
> **Stack:** Rust 2021, `egui 0.35 + eframe 0.36` (no webview, verified 2026-08-29: `egui 0.35.0` / `eframe 0.36.1` / `crates.io`), `tokio`, `core` lib + `app` + `cli` workspace
> **Legacy:** `src/main.rs:1237` + `src/tui.rs:2691` (ratatui/crossterm) will NOT be migrated — fresh start, no backwards compatibility
> **AI:** Extensible `AiProvider` registry — `GeminiProvider` is the first provider (native video verified `ai.google.dev/gemini-api/docs/video-understanding`), future providers pluggable
> **Date:** 2026-08-29 (crate versions live-verified via crates.io/docs.rs/lib.rs)

---

## Table of Contents

1. [Vision & Non-Goals](#1-vision--non-goals)
2. [Why egui](#2-why-egui)
3. [Architecture Overview](#3-architecture-overview)
4. [Workspace Layout](#4-workspace-layout)
5. [AI Provider Handler (Extensible)](#5-ai-provider-handler-extensible)
6. [Core Subsystems](#6-core-subsystems)
7. [App (egui) UX Flow](#7-app-egui-ux-flow)
8. [Security (Redesigned)](#8-security-redesigned)
9. [Setup & Toolchain](#9-setup--toolchain)
10. [CLI](#10-cli)
11. [Testing Strategy](#11-testing-strategy)
12. [CI/CD & Packaging](#12-cicd--packaging)
13. [i18n, Logging, Error Handling](#13-i18n-logging-error-handling)
14. [Milestones — Definition of Done per Milestone](#14-milestones--definition-of-done-per-milestone)
15. [Acceptance Criteria for v2.0.0](#15-acceptance-criteria-for-v200)
16. [Risks & Mitigations](#16-risks--mitigations)
17. [Appendix: Legacy Mapping](#17-appendix-legacy-mapping)

---

## 1. Vision & Non-Goals

**Vision:** A lightweight, well-maintained desktop app for Windows & Linux that downloads a YouTube video, splits it, analyzes it with AI for best moments, lets the user review/edit moments on a timeline, and exports vertical 9:16 shorts via a declarative `Plano` compositing system — all with a native egui GUI, no Tauri/Electron/HTML.

**Non-Goals:**

- No webview, no Tauri/Electron.
- No extreme Win32/GTK dual-native UIs (egui look is acceptable; lightest + best maintained wins).
- No retro-compatibility with v1 `settings.json` (`src/config.rs:177` path) — fresh config at `config_dir/yt-shortmaker-v2/`.
- No support for non-native-video providers at launch — trait is ready, implementation is Gemini-only.

---

## 2. Why egui

| Criterion | egui 0.35 + eframe 0.36 (verified) | Slint 1.17.1 (verified) | Iced 0.14.0 (verified) |
|---|---|---|---|
| Version (verified 2026-08-29) | `egui 0.35.0` (2026-06-25) / `eframe 0.36.1` (2026-08-07), crates.io 14M DL, 58 versions, Rerun sponsored | `slint 1.17.1` (2026-07-07), lib.rs #16 GUI, 87k DL/mo, 48 releases stable, GPL-3.0/Royalty-free | `iced 0.14.0` docs.rs, GitHub 31k stars / 6.8k commits, used by COSMIC/Kraken |
| Webview | No | No | No |
| Binary size | ~6-8 MB stripped | ~10-12 MB | ~18-25 MB |
| Startup | <200 ms | <300 ms | <400 ms |
| Look | Own style (themed) — `egui` docs: "If you want native look, egui is not for you" | Closest to native | Own style |
| Language | Pure Rust (immediate mode) | Rust + `.slint` DSL | Pure Rust (Elm) |
| Learning cost | Low | Medium (DSL) | Medium |

Decision: **egui** — lightest, most maintained (monthly releases, `CHANGELOG 0.36.1`), fastest iteration, zero webview. `core` is UI-agnostic so a future Slint/Iced migration is possible without touching business logic. Alternatives remain viable and verified as well-maintained.

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────┐
│  crates/app  (egui + eframe)            │
│  - MainWindow, SidePanel, CentralPanel  │
│  - AppState (view-model)                │
│  - tokio bridge -> ctx.request_repaint()│
└────────────────┬────────────────────────┘
                 │ calls async via mpsc
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

- **Pattern:** MVVM + message passing. `core` exposes `async fn` pure functions. `app` holds `AppState` (`Arc<Mutex<>>`) and dispatches `tokio::spawn` tasks, forwarding progress via `UnboundedSender<AppEvent>` -> `egui::Context::request_repaint()`.
- **Cancellation:** `tokio_util::CancellationToken` (replaces `AtomicBool` polling `src/video.rs:79`).
- **Error:** `anyhow` in `core` + `thiserror` typed errors at boundaries, surfaced as `AppEvent::Error`.

---

## 4. Workspace Layout

```text
yt-shortmaker-v2/
  Cargo.toml              # [workspace] members = ["crates/core","crates/app","crates/cli"]
  Cargo.lock
  rust-toolchain.toml     # channel = "stable"
  deny.toml / clippy.toml
  build.rs                # winresource 0.1.31 (replaces unmaintained winres 0.1.12 since 2021) icon only (evolution of build.rs:48), prebuilt logo.ico
  assets/
    icon.png              # source for icon (logo.png)
  locales/
    en.yml / es.yml / ru.yml  # from locales/*.yml:197
  PLAN.md                 # this file
  docs/
    ARCHITECTURE.md
    PLANO_SPEC.md
  crates/
    core/
      Cargo.toml
      src/
        lib.rs
        config/{mod.rs,store.rs}
        types/mod.rs
        media/{mod.rs,ffmpeg.rs,ytdlp.rs,pipeline.rs}
        ai/{mod.rs,provider.rs,gemini.rs}
        plano/{mod.rs,schema.rs,compiler.rs,preview.rs}
        session/mod.rs
        security/{mod.rs,keyring.rs}
        setup/mod.rs
    app/
      Cargo.toml
      src/
        main.rs
        state/{mod.rs,bridge.rs}
        actions/{mod.rs,analyze.rs,export.rs}
        ui/{mod.rs,components.rs}
    cli/
      Cargo.toml
      src/main.rs         # clap 4
```

---

## 5. AI Provider Handler (Extensible)

**Concept:** Shop with one supplier today, ready for more. Gemini is the first provider with native video; others plug in via the same trait.

```rust
// crates/core/src/ai/provider.rs
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &'static str;           // "gemini"
    fn display_name(&self) -> &'static str; // "Google Gemini"
    fn capabilities(&self) -> ProviderCapabilities;
    async fn analyze_chunk(&self, ctx: AnalyzeCtx) -> Result<Vec<VideoMoment>>;
    async fn test_connection(&self) -> Result<()>;
}

pub struct ProviderCapabilities {
    pub supports_native_video: bool,
    pub supported_mimes: Vec<&'static str>, // e.g. video/mp4, video/webm
    pub max_video_duration: Duration,
    pub max_inline_size_mb: u32,
}

pub struct AnalyzeCtx {
    pub chunk_path: PathBuf,
    pub chunk_start_offset: Duration,
    pub cancellation: CancellationToken,
    pub progress_tx: Option<mpsc::UnboundedSender<ProviderEvent>>,
}

// crates/core/src/ai/mod.rs
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
    active_id: String,
}

impl ProviderRegistry {
    pub fn new(active_id: String) -> Self { /* ... */ }
    pub fn register(&mut self, provider: Arc<dyn AiProvider>) { /* ... */ }
    pub fn active(&self) -> Arc<dyn AiProvider> { /* ... */ }
    pub fn get(&self, id: &str) -> Option<Arc<dyn AiProvider>> { /* ... */ }
    pub fn list(&self) -> Vec<Arc<dyn AiProvider>> { /* ... */ }
}
```

**Gemini implementation:** `crates/core/src/ai/gemini.rs` — `GeminiProvider` (`supports_native_video = true` verified `ai.google.dev/gemini-api/docs/video-understanding` MIME `video/mp4` etc, 1M context; vs `GPT-4o via Whisper+keyframes` / `Claude limited` per 2026 benchmarks), uses `upload/v1beta/files` resumable + `wait_for_file_active` + `models/{model}:generateContent` with `responseSchema` structured output (`src/gemini.rs:158-494`). Handles quota `429/RESOURCE_EXHAUSTED` by marking key disabled + rotating.

**Future providers:** `OpenAiProvider`, `AnthropicProvider` etc implement the same trait (only if/when they gain native video; today `supports_native_video = false`). If `supports_native_video == false`, `pipeline.rs` will either error with guidance or fallback to frame-extraction + Whisper (explicit, not silent). UI shows capability badge.

**Config shape (v2, no v1 compat):**

```json
{
  "version": 2,
  "ai": {
    "active_provider": "gemini",
    "providers": {
      "gemini": {
        "keys": [{ "name": "Primary", "value": "keyring:gemini_primary", "enabled": true }],
        "model": "gemini-flash-latest"
      }
    }
  }
}
```

---

## 6. Core Subsystems

### 6.1 Config

- Path: `dirs 6.0.0` (verified 2025-01-12, 306M DL, `dirs::config_dir()`) or `directories 6.0.0` at `yt-shortmaker-v2/settings.json` (fresh, not `src/config.rs:170`). `directories` preferred if `project_path` needed.
- `AppConfig`, `ShortsConfig` unified into `Plano`-first config. `ShortsConfig` legacy (`src/config.rs:28`) removed — replaced by default `Plano`.
- `store.rs` handles load/save with `serde 1.0.229` + `serde_json 1.0.151` (verified 1.3B/1.2B DL) + `jsonschema` validation, atomic write.

### 6.2 Types

- `VideoMoment { start_time: String (HH:MM:SS), end_time, category, description, dialogue: Vec<DialoguePhrase> }` (`src/types.rs:15`).
- `VideoChunk { start_seconds: u64, file_path: String }` (`src/types.rs:26`).
- `SessionState` replaced by DB entities (see 6.6).

### 6.3 Media

- **ffmpeg.rs:** `get_duration`, `get_duration_precise`, `get_resolution`, `FilterCompiler::build(&[PlanoObject], clip_path) -> (filter_complex, inputs)` — single compiler unifying `src/shorts.rs:72` + `src/exporter.rs:397`. Consider `ffmpeg-sidecar 2.5.2` (verified 1.7M DL) vs manual `tokio::process::Command` + `ffprobe` — manual preferred for transparency; `ffmpeg-next 9.0.0` bindings avoided (heavy).
- **ytdlp.rs:** `extract_video_id` (`src/video.rs:16`), `validate_media_url` (`src/video.rs:493`), `download_low_res` (`src/video.rs:153`), `download_high_res` (`src/video.rs:219`) — all async with `CancellationToken` (`tokio 1.53.1` verified, 910M DL) + `reqwest 0.13.4` (verified, 668M DL; replaces 0.12) + retry with exponential backoff (replaces manual `attempt` loop `src/video.rs:259`).
- **pipeline.rs:** `enum Stage { Downloading, Splitting, Analyzing { current, total }, Extracting }` + `Pipeline::run(ctx, callbacks) -> Result<RunOutput>` using `tokio-util 0.7.19` (740M DL) + `futures-util 0.3.34` (856M DL).

### 6.4 Plano

- **schema.rs:** `PlanoObject::{Clip, Image, Shader { Blur }, Video}` (`src/exporter.rs:188`), `Position`, `SizeValue`, `PositionValue`, `Fit::{Cover,Contain,Stretch}`, `Crop`, `Opacity`.
- **compiler.rs:** `build_ffmpeg_filter` (`src/exporter.rs:397`) with `loop`, `tpad`, `boxblur`, `scale/crop/pad`, `colorchannelmixer` handling.
- **preview.rs:** `generate_preview(plano, source)` returns `image::DynamicImage` / `egui::ColorImage` bytes (replaces file-path `src/exporter.rs:636` + `open::that` `src/tui.rs:1287`).
- Default plano: 3 layers (blurred bg Clip + Shader Blur + main Clip centered) (`src/exporter.rs:319`).

### 6.5 AI

- See §5. `gemini.rs` ported with `tracing` instead of `eprintln`, key rotation via `Mutex`, not `AtomicUsize` alone.

### 6.6 Session

- `rusqlite 0.40.2` (verified 2026-08-08, 97M DL, SQLite 3.53.2 bundled) at `config_dir/yt-shortmaker-v2.db` with `features = ["bundled"]`.
- Tables: `projects(id, url, video_id, title, duration, status, created_at, updated_at)`, `chunks(project_id, idx, start_sec, path)`, `moments(project_id, idx, start_time, end_time, category, description, dialogue_json)`, `exports(id, project_id, plano_json, output_dir, created_at)`.
- Replaces `temp.json` (`src/main.rs:459`) + `cache_{id}` (`src/main.rs:551`).

### 6.7 Security

- Redesigned, `Simple` obfuscation (`src/security.rs:51` hardcoded key) **removed**.
- Modes: `None` (plain, dev), `Keyring` (default, `keyring 3.6.3 / 4.1.2` verified 2026-06-21, `open-source-cooperative/keyring-rs`, lib.rs #4 Auth 2.2M DL/mo -> OS Credential Manager / Secret Service / Keychain), `Password` (`aes-gcm 0.11.1` verified 143M DL, up from 0.10.3 + `argon2 0.5.3` stable / `0.6.0-rc.8` 47M DL + `zeroize 1.8`/`base64 0.23.1` 1.4B DL, zeroized).
- `settings.json` stores `keyring:` refs, not raw keys.

### 6.8 Setup

- `check_dependencies` (`src/video.rs:23`) + `ensure_tools` (`src/setup.rs:42`) with egui `ProgressBar`, `reqwest 0.13.4` stream download, `zip 9.0.0` (verified 251M DL; replaces legacy `zip 0.6` from `Cargo.toml:40`) extract for Windows (`src/setup.rs:248`), `PermissionsExt` for Unix (`src/setup.rs:168`), `add_to_process_path` (`src/setup.rs:282`).

---

## 7. App (egui) UX Flow

```
Home (recent projects from DB)
 └─ New Project
    ├─ [1 Ingest] URL input + Paste + Fetch Info (yt-dlp --dump-json) -> title/duration/thumbnail
    ├─ [2 Analyze] Config (chunk size, model fast/pro, provider, keys) -> Download low-res -> Split -> AI per chunk (sticky key) -> Save DB
    ├─ [3 Review] Editable moments table (TextEdit start/end, ComboBox category, description, expandable dialogue) + thumbnail scrubber + Add/Delete/Re-analyze chunk
    ├─ [4 Queue] Select moments -> Add to Export Queue (choose Plano)
    └─ [5 Export] Plano Editor (LayerList drag&drop + Inspector: position/size/crop/fit/opacity) -> Live Preview (egui Texture) -> Batch -> Done (open folder, metrics)
Export Standalone (no AI) -> pick clips + Plano + Preview -> same [5]
Settings (OutputDir, cookies, language, ffmpeg path, ai provider/keys, theme)
Keys (CRUD per provider, Test button)
```

Replaces linear `MainMenu -> UrlInput -> FormatConfirm (placeholder src/main.rs:568) -> Processing -> ShortsConfirm -> Done` (`src/main.rs:495`) and separate `ExportShorts` (`src/tui.rs:98`). Moments are now editable (vs read-only `src/tui.rs:2125`), preview is embedded (vs external viewer `src/tui.rs:1041`).

**egui structure:**

```rust
// crates/app/src/main.rs
fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    eframe::run_native("yt-shortmaker", opts, Box::new(|cc| Box::new(App::new(cc, rt.handle().clone()))))
}

// crates/app/src/state/mod.rs
struct AppState { projects: Vec<Project>, current: Option<ProjectState>, export_queue: Vec<QueueItem>, settings: AppConfig, logs: Vec<LogEntry> }
enum AppEvent { Progress(f32,String), Log(LogLevel,String), MomentFound(VideoMoment), Complete(String), Error(String), Finished }
```

---

## 8. Security (Redesigned)

- Threat model: casual file read, not nation-state.
- `Keyring` is default; `Password` is opt-in with `zeroize`.
- No `SIMPLE_KEY_BYTES` (`src/security.rs:51`).

---

## 9. Setup & Toolchain

- `ffmpeg` + `yt-dlp` auto-download to `data_dir/yt-shortmaker-v2/bin` with egui progress, or use system PATH if found.
- `rust-toolchain.toml` stable, `clippy.toml`, `deny.toml`.

---

## 10. CLI

`crates/cli` with `clap 4.6.6` (verified 2026-08-06, 1B DL):

- `yt-shortmaker-cli preview <video> [timestamp] --plano plano.json` (from `src/main.rs:97`)
- `yt-shortmaker-cli transform <video> [out] --plano ...` (from `src/main.rs:131`)
- `yt-shortmaker-cli batch <in_dir> [out_dir] --plano ...` (from `src/main.rs:180`)
- `yt-shortmaker-cli analyze <url> --provider gemini` (new, headless AI)
- Shares `core` 100%.

---

## 11. Testing Strategy

- **Unit (core):** config store, plano compiler snapshots, ffmpeg filter strings, gemini JSON parsing (`src/gemini.rs:576`), video helpers (`src/video.rs:509`), provider capabilities.
- **Integration:** real `ffmpeg` with tiny fixture mp4, `yt-dlp --dump-json` mock, `gemini` via `wiremock`.
- **UI:** `egui_kittest` / `egui-test` — click, type, assert state; screenshot diff for preview.
- **E2E:** `cargo run -p cli -- batch fixtures/clips --plano fixtures/plano.json` + `ffprobe` assert.
- Gates: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo deny check`, `cargo test`, `cargo test -- --ignored` for e2e.

---

## 12. CI/CD & Packaging

**Evolves `.github/workflows/release.yml:6` + `wix/main.wxs` + `Cross.toml:1`:**

- Matrix: `x86_64-pc-windows-msvc` (exe + MSI via `cargo-wix 0.3.9` verified 2025-03-13, requires WiX 3.14 legacy; for Modern WiX v4+ add explicit `wixtoolset/wix` install step), `x86_64-unknown-linux-gnu` (binary + `.deb` via `cargo-deb 3.7.0` verified 2026-05-02 + `AppImage`), optional `aarch64-unknown-linux-gnu` via `cross 0.2.5` (verified crates.io 2023-02-04, 6.2M DL — stale publish; consider `cargo cross` GitHub `cross-rs/cross` main branch if `aarch64` fails).
- Remove `i686`/`aarch64-win` unless demand.
- `build.rs`: only `winresource 0.1.31` icon, prebuilt `logo.ico` (remove `winres 0.1.12` unmaintained since 2021 + `image 0.24` resize `build.rs:22` -> `image 0.25.10` verified 2026-03-10 if needed).
- Linux deps: `rustls` instead of `openssl vendored` (`Cargo.toml:9`) to avoid `libssl` cross issues.
- Version from `Cargo.toml` workspace + `git-cliff`.

---

## 13. i18n, Logging, Error Handling

- **i18n:** `rust-i18n 4.2.1` (verified 2026-07-16, 3.1M DL; replaces `3.1.5` from `Cargo.toml:15`) + `locales/*.yml`, expose `tr!("key")` to egui. Languages: `en/es/ru`.
- **Logging:** `tracing 0.1.44` + `tracing-subscriber 0.3.23` (verified 804M/574M DL) + `tracing-appender` (rotating file `data_dir/logs/yt-shortmaker.log`), replaces `simplelog 0.12.2` (verified 2024-03-04, stale) + `debug.log` (`src/main.rs:55`).
- **Error:** `thiserror 2.0.20` (verified 2026-08-08, 1.3B DL; replaces 1.x) typed + `anyhow 1.0.104` (910M DL) at top, surfaced in egui as toast + `Done` error panel (replaces `tui.rs:2125` log list). Shared helpers `chrono 0.4.45` (765M DL), `regex 1.13.1` (1B DL), `base64 0.23.1`, `dirs` remain.

---

## 14. Milestones — Definition of Done per Milestone

### Milestone 0 — Bootstrap & POC (4 days)

- [ ] New workspace `yt-shortmaker-v2` created (`Cargo.toml` workspace, `rust-toolchain.toml`, `deny.toml`, `clippy.toml`).
- [ ] `egui` POC window runs on Windows 10/11 and Ubuntu 24.04 (`cargo run -p app` shows `CentralPanel` + button).
- [ ] CI `test.yml` green (fmt, clippy, test).
- [ ] `PLAN.md` + `docs/ARCHITECTURE.md` committed.

**DoD:** `cargo run -p app` opens native egui window on both OSes; CI passes.

### Milestone 1 — Core Crate (10 days)

- [ ] `core::config` — `AppConfig` v2 store (load/save, validation, tests).
- [ ] `core::types` — `VideoMoment`, `DialoguePhrase`, `VideoChunk` ported with tests.
- [ ] `core::media::ffmpeg` — `get_duration`, `get_resolution`, `FilterCompiler` (unified, snapshot tests).
- [ ] `core::ai::provider` — `AiProvider` trait + `ProviderRegistry` + `ProviderCapabilities` + unit tests.
- [ ] `core::ai::gemini` — `GeminiProvider` (native video) with `wiremock` tests.
- [ ] `core::plano` — schema + compiler + preview (returns `ColorImage`) + tests.
- [ ] `core::session` — `rusqlite` DB (projects/moments/chunks) + tests.
- [ ] `core::security` — `None/Keyring/Password` modes + `keyring` integration + tests.
- [ ] `core::setup` — `check_dependencies` + `ensure_tools` (unit, no UI).

**DoD:** `cargo test -p core` >80% coverage, no `core` dep on `egui`.

### Milestone 2 — Media Pipeline (7 days)

- [ ] `core::media::ytdlp` — async download low/high with `CancellationToken` + retry + tests (mock).
- [ ] `core::media::pipeline` — `Stage` state machine + `Pipeline::run` (Download→Split→Analyze→Extract) + cancellation + progress callbacks.
- [ ] `setup` download with progress channel (tested with `wiremock` server).
- [ ] `tracing` logging to rotating file.

**DoD:** `Pipeline::run` with mocked AI processes a 2-min fixture video end-to-end (no real Gemini) and produces chunks + moments in DB.

### Milestone 3 — App Shell (10 days)

- [ ] `app` egui shell: `MainWindow` with `SidePanel` (Home / New Project / Export Standalone / Settings / Keys) + `CentralPanel` router.
- [ ] `Home` — recent projects from DB (list, open, delete).
- [ ] `Settings` — output dir (native folder picker `rfd`), language, ffmpeg path, theme, live-save.
- [ ] `Keys` — per-provider CRUD (add/rename/toggle/delete), `Test` button calls `test_connection`.
- [ ] i18n wired to egui (`en/es/ru`).

**DoD:** User can launch app, see Home, edit Settings, manage Gemini keys via OS keyring, and settings persist across restarts.

### Milestone 4 — Analyze & Review Flow (12 days)

- [ ] `Ingest` page — URL input + validation (`validate_media_url`), `Fetch Info` (yt-dlp `--dump-json` -> title/duration/thumb), `thumbnail` display.
- [ ] `Analyze` page — config (chunk size, model, provider), `Start` -> pipeline with `ProgressBar`, virtualized log view, `Cancel` via `CancellationToken`.
- [ ] `Review` page — editable moments table (start/end `TextEdit` with `HH:MM:SS` validation, category `ComboBox`, description, expandable dialogue), `Thumbnail` scrubber (`ffmpeg -ss` frame -> `Texture`), `Add` manual moment, `Delete`, `Re-analyze chunk`.
- [ ] Persistence — moments saved to DB, export to `moments.json`/`moments.txt` (from `src/main.rs:1078`).

**DoD:** User can paste URL, run Analyze (mock or real Gemini), see moments in Review, edit a moment's timestamps/category, and see thumbnail update.

### Milestone 5 — Export & Plano Editor (12 days)

- [ ] `Plano` Editor — `LayerList` (drag&drop reorder) + `Inspector` (position x/y, size w/h, `Fit`, `Crop`, `Opacity`, `loop_video`), `Plano` JSON load/save with `//` comments (`src/exporter.rs:285`).
- [ ] `Preview` — `plano::preview` -> `egui Texture` embedded (no external viewer), updates on Plano change.
- [ ] `Export` page — `Batch` queue (select clips or Review selection), `OutputDir` picker, naming template (`short_%Y%m%d_%H%M%S`), `Start` -> `export_batch` with per-clip progress + `Cancel`.
- [ ] `Done` — metrics (clips count, time), `Open Folder` (native `open` crate `Cargo.toml:32`), `New Project`.

**DoD:** User can pick clips + edit Plano (e.g., change blur intensity, main height), see live preview, run Batch export, and get 9:16 shorts in output dir verified by `ffprobe` 1080x1920.

### Milestone 6 — CLI & Polish (5 days)

- [ ] `crates/cli` — `preview`, `transform`, `batch`, `analyze` (clap) sharing `core`.
- [ ] Polish — keyboard shortcuts, tooltips, empty states, onboarding first-run (API key prompt), theme light/dark.
- [ ] Docs — `README.md` updated (replaces `README.md:60` cargo run), `docs/PLANO_SPEC.md`.

**DoD:** `cargo run -p cli -- preview fixtures/clip.mp4 --plano fixtures/plano.json` produces `preview.png`; `cargo run -p cli -- batch fixtures/clips` produces shorts; README instructions work on clean machine.

### Milestone 7 — Packaging & QA (7 days)

- [ ] `release.yml` — matrix for Win exe+MSI, Linux binary+deb+AppImage, cross for aarch64.
- [ ] `build.rs` — prebuilt `logo.ico` only.
- [ ] E2E tests — `ffmpeg` real, `ffprobe` 1080x1920 assert, `cargo test --ignored`.
- [ ] Manual QA — Windows 11 + Ubuntu 24.04 (fresh VM, no ffmpeg), verify auto-download + fallback PATH.
- [ ] `vm.sh` updated.

**DoD:** `cargo build --release` on both OSes + `cargo wix` / `cargo-deb` produce installers; manual QA checklist passes.

### Milestone 8 — Beta Release (3 days)

- [ ] Tag `v2.0.0`, `release.yml` release notes (replaces `release.yml:140`), `FUNDING.yml` check.
- [ ] Beta announcement + feedback issue template.
- [ ] Archive v1 branch.

**DoD:** GitHub Release `v2.0.0` with assets `yt-shortmaker-windows-amd64.exe/.msi`, `yt-shortmaker-linux-amd64` + `deb`/`AppImage`, release notes published.

---

## 15. Acceptance Criteria for v2.0.0

- [ ] All Milestones 0-8 DoD met.
- [ ] `cargo test` passes on Windows + Linux; `cargo clippy -- -D warnings` clean.
- [ ] No `ratatui`/`crossterm` code remains; no `Simple` encryption; no `temp.json` global.
- [ ] One-command install: download MSI/DEB or exe/binary, double-click, no manual ffmpeg/yt-dlp for 90% users.
- [ ] User journey `URL -> Analyze (Gemini) -> Review (edit) -> Export (Plano) -> Shorts` works end-to-end with real Gemini key.
- [ ] Binary size <15 MB stripped, startup <1s.

---

## 16. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `egui` immediate-mode complexity for large tables | Use `egui_extras 0.36.1` (`TableBuilder` verified) + virtualization, `egui::ScrollArea` |
| `ffmpeg` not found | `setup::ensure_tools` with clear error + `open 5.4.2` (49M DL) docs link; fallback to system PATH |
| Gemini quota 429 | `ProviderRegistry` tracks disabled keys, UI health badge, exponential backoff (`tokio 1.53.1`) |
| `keyring` Secret Service not available (headless Linux) | Fallback to `Password` mode (`aes-gcm 0.11.1`/`argon2 0.5.3`) with prompt, or `None` with warning |
| `rusqlite 0.40.2` bundling `libsqlite3` | Use `bundled` (SQLite 3.53.2), or `sqlx` alternative if `cross 0.2.5` stale fails |
| `winres` unmaintained / `cross` stale | Mitigated: `winresource 0.1.31` + `cargo-wix` WiX legacy note; `cross` fallback to `cargo xwin` or native build |

---

## 17. Appendix: Legacy Mapping

| Legacy (`v1`) | v2 Equivalent |
|---|---|
| `src/main.rs:90` CLI `preview/transform/batch` | `crates/cli` |
| `src/tui.rs:144` `App` God object | `crates/app/src/state` ViewModel |
| `src/tui.rs:62` `AppScreen` 16 variants | `egui` router (`SidePanel` + `CentralPanel` enum) |
| `src/config.rs:28` `ShortsConfig` | Removed — `Plano` only |
| `src/shorts.rs:72` `build_filter_complex` | Unified `core::media::ffmpeg::FilterCompiler` |
| `src/exporter.rs:397` `build_ffmpeg_filter` | Same unified compiler |
| `src/gemini.rs:158` `GeminiClient` | `core::ai::gemini::GeminiProvider` via `AiProvider` |
| `src/security.rs:51` `SIMPLE_KEY_BYTES` | Removed |
| `src/video.rs:23` `check_dependencies` | `core::setup` |
| `src/setup.rs:42` `run_setup_wizard` (ratatui) | `core::setup` + egui `ProgressBar` |
| `src/types.rs:32` `SessionState` file | `core::session` DB |
| `build.rs:22` `image 0.24` resize | Prebuilt `logo.ico` (`image 0.25.10` if runtime needed) |
| `locales/*.yml` (`rust-i18n 3.1.5`) | Kept, wired to egui `tr!` (`rust-i18n 4.2.1`) |
| `wix/main.wxs` (`cargo-wix 0.3.9` legacy WiX 3.14) | Kept, adapted for new binary name; add Modern WiX v4 step if needed |
| `Cross.toml:1` (`cross 0.2.5` stale 2023) | Kept for aarch64 but verify; fallback to native `aarch64` runner |

---

*Plan generated from full audit of `Cargo.toml`, `src/*.rs`, `locales/*.yml`, `.github/workflows/*.yml`, `build.rs`, `wix/main.wxs`, `Cross.toml`.*
*Live verification 2026-08-29 (all crates via crates.io API/docs.rs/lib.rs unless noted):*
*GUI: `egui 0.35.0/eframe 0.36.1/egui_extras 0.36.1` (14-17M DL), `slint 1.17.1` (#16 GUI 87k/mo), `iced 0.14.0` (31k stars) | Core: `tokio 1.53.1` (910M DL), `reqwest 0.13.4` (668M), `rusqlite 0.40.2` (97M, SQLite 3.53.2), `serde 1.0.229/serde_json 1.0.151` (1.3B), `clap 4.6.6` (1B), `chrono 0.4.45` (765M), `dirs 6.0.0/directories 6.0.0`, `tracing 0.1.44/tracing-subscriber 0.3.23` (804M/574M), `thiserror 2.0.20/anyhow 1.0.104` (1.3B/910M), `regex 1.13.1` (1B), `base64 0.23.1` (1.4B), `futures-util 0.3.34/tokio-util 0.7.19` | Security: `keyring 3.6.3/4.1.2` (#4 Auth 2.2M/mo), `aes-gcm 0.11.1` (143M), `argon2 0.5.3/0.6.0-rc.8` (47M), `zeroize` | Media: `image 0.25.10` (177M), `zip 9.0.0` (251M, was 0.6), `ffmpeg-sidecar 2.5.2`, `rfd 0.17.2` (26M), `open 5.4.2` (49M), `notify-rust 4.18.0` | Packaging: `winresource 0.1.31` replaces unmaintained `winres 0.1.12` (2021), `cargo-wix 0.3.9` (WiX 3.14 legacy), `cargo-deb 3.7.0`, `cross 0.2.5` (stale 2023, flagged), `rust-i18n 4.2.1` (was 3.1.5) | AI: `Gemini native video` (ai.google.dev docs: `gemini-2.5-flash` 1M ctx, 10 videos/req, 20GB File API, `media_resolution`, vs GPT-4o via Whisper+keyframes / Claude limited per 2026 benchmarks).*
