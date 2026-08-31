# AGENTS — yt-shortmaker v2

> This file is read by **any AI agent** touching the repo. Violating it breaks the build or the UX. Read `STYLE.md` and `PLAN.md` before touching `crates/app/ui/app.slint` or `crates/core`.

## 1. What it is for

**yt-shortmaker v2** (workspace `2.0.0`, `Cargo.toml:1`) downloads a long YouTube video, splits it into chunks, analyzes each chunk with AI (Gemini native video) to detect the best moments, and exports vertical 9:16 clips via **Plano** (declarative compositing that compiles to ffmpeg filters). Native **Slint 1.17** app (`crates/app`), no webview; `crates/core` is 100% UI-agnostic; `crates/cli` is headless/batch with `clap 4.6`.

User flow: `URL → Fetch Info (ytdlp) → Analyze (Gemini File API + generateContent) → Review (table, editable) → Export (Plano) → shorts/*.mp4` + `moments.json/txt`. SQLite session is video-centric and allows resume by video_id with folder+DB verification. No projects list.

Stack: Rust 2021 stable (`rust-toolchain.toml`), `tokio 1.53` + `tokio-util CancellationToken`, `anyhow`/`thiserror`, `rusqlite 0.40 bundled`, `keyring 3.6`, `rust-i18n 4.2`, `tracing` (ansi disabled).

## 2. How it works (architecture)

```
crates/app (Slint 1.17)          crates/core (no UI)            externals
┌──────────────────────┐         ┌─────────────────────┐        ┌──────────────┐
│ ui/app.slint         │────────▶│ config/store        │        │ ffmpeg/ffprobe│
│  MainWindow (router) │  calls  │ media/{ytdlp,       │───────▶│ yt-dlp        │
│  route=home|export|  │  core   │  ffmpeg,chunk,      │ spawns │ Gemini API    │
│  settings|keys       │  sync+  │  pipeline}          │        │ (File API +   │
│ main.rs callbacks    │  async  │ ai/{provider,       │        │  generateContent)
│  Rc<RefCell<AppConfig>> +     │  gemini, registry}  │        └──────────────┘
│  tokio + channels    │         │ plano/{schema,       │
└──────────────────────┘         │  compiler,preview}  │
                                 │ session (SQLite)    │
                                 │ security/keyring    │
                                 │ setup (deps)        │
                                 │ types (VideoMoment) │
                                 └─────────────────────┘
```

**Core (lib.rs:5):** `config | media | ai | plano | session | security | setup | types | util`. Pattern: `core` exposes pure functions; `app` holds `Rc<RefCell<AppConfig>>` (`crates/app/src/main.rs:366`) and Slint in/out properties; long work runs in `std::thread::spawn + tokio::Runtime::new + CancellationToken` with progress via `invoke_from_event_loop`.

- **config (`crates/core/src/config/mod.rs:117`)**: `AppConfig { version:2, language, output_dir, cookies, processing{chunk_size_secs,max_last_chunk,delay,retry,auto_extract}, export{naming_template,ffmpeg_path}, ai{active_provider, providers{keys,model,model_pro,use_fast_model,temperature,media_resolution}, custom_models[], active_model_id} }`. Path `dirs::config_dir()/yt-shortmaker-v2/settings.json` (`store.rs` via `config::APP_DIR_NAME`), atomic tmp+rename write, `normalize()` validates ranges (`max_last_chunk` now `60..=3600`, not `> chunk_size`) and migrates `model/model_pro` → `custom_models`. Default `chunk_size=600, max_last=900` no longer triggers spurious normalize. `slugify` now delegates to `util::slugify` (`config/mod.rs:515`, `slugify_pub` for app).
- **CustomModel (`config/mod.rs:256`)**: `{id,display_name,base_provider,model_id,temperature,media_resolution,enabled}`. Allows any `model_id` on `base_provider="gemini"` (e.g. `gemma-4-26b-a4b-it` via Gemini API) without a code update.
- **media**: `ffmpeg.rs` `get_duration/get_resolution` + `setup::dependency_status`; `chunk.rs:21` `calculate_chunks()` + `split_video` (`-ss/-t` with cancellation); `ytdlp.rs:49` `fetch_info/download_video` + `extract_video_id` (YouTube 11-char `v=|/` + Twitch `twitch.tv/videos/{id} -> v{id}` + generic last-segment + `hash_url` `url_{fnv1a}` fallback, never timestamp), constants `LOW_RES_FORMAT`/`HIGH_RES_FORMAT` (shared with `pipeline.rs`), helpers `fnv1a_hash/hash_url` -> `util::fnv1a_hash` and `exponential_backoff` via `util::exponential_backoff`; `pipeline.rs` state machine `Stage::Downloading|Splitting|Analyzing{current,total}|Extracting` + `Pipeline::run` with stable id `session::resolve_stable_id(url)` cache-first (`video_cache` exact -> `video_id` variant -> `extract` -> `hash_url`), throttled `emit_ytdlp_line` (1Hz + dedup, progress 0.5%) and `PipelineEvent::Progress/Log/StageChanged`, `check_dependencies` fail-fast, strict verification (folder+DB).
- **ai (`ai/provider.rs:42`, `ai/gemini.rs:53`)**: `trait AiProvider { id, display_name, capabilities{supports_native_video}, analyze_chunk, test_connection }`. `ProviderRegistry HashMap<String, Arc<dyn AiProvider>>`. `GeminiProvider { keys, disabled, cursor }` resolves `AiConfig::resolved_model()/resolved_base_provider()` (`config/mod.rs:236`) → `generativelanguage.googleapis.com/v1beta`. Per-chunk sticky key: `analyze_with_key:278` reuses same key for `upload_file:118` + `wait_for_file_active:160` + `generate_moments:198`; on `429/RESOURCE_EXHAUSTED` (`is_quota_error:424`) marks key disabled and retries same chunk with next key via `next_key_index:98` (round-robin skipping disabled). Never mixes keys for same `file_uri`.
- **plano**: `PlanoObject::{Clip,Image,Shader{Blur},Video}` → `build_ffmpeg_filter` (blurred bg + `Shader Blur` + centered `Clip`). Replaces `ShortsConfig` v1 (blur/zoom/height/opacity now in `plano/*.json`).
- **session (`session/mod.rs:17`)**: `rusqlite` at `data_local_dir()/yt-shortmaker-v2/session.db` (`config::APP_DIR_NAME`) tables `video_jobs|job_chunks|job_moments|video_cache` (legacy `projects|chunks|moments|exports` dropped on `init_db:97`). `video_cache` for `fetch_info` (url PK + `video_id` index) with `img_cache/` next to DB. `video_jobs` is checkpoint state per `video_id` (`work_dir, download_path, download_verified, split_verified, total_chunks, analyzed_chunks, status`), `job_chunks` per idx with `status`, `job_moments` appended per chunk. No projects list. Verification is folder+DB: download exists + `ffmpeg::get_duration`, split chunks exist + durations, analyze strict order (must not skip `0..i-1`). Helpers `video_work_dir_or_tmp` (central fallback vs `temp_dir`), `video_download_path`, `resolve_stable_id(url)` (cache -> extract -> hash), `cached_video_for_url(url)` (url exact -> id variant).
- **security (`security/mod.rs`)**: `None` (dev) / `Keyring` (default, `keyring 3.6`) `set_secret/get_secret/delete_secret("yt-shortmaker", name)`. No `Password`/`Secret` zeroize until implemented.
- **setup (`setup/mod.rs`)**: `tools_dir() %LOCALAPPDATA%/yt-shortmaker-v2/tools` via `config::APP_DIR_NAME`, `yt_dlp_bin/ffmpeg_bin/ffprobe_bin`, `dependency_status/is_ok/missing`, `ensure_tools_with_progress(Fn(String))` (reqwest `bytes_stream` + `content_length` with `%`, `zip deflate` for FFmpeg `BtbN/FFmpeg-Builds`), `AtomicBool INSTALLING` prevents races.
- **util (`util.rs`)**: `fnv1a_hash/hash_url`, `exponential_backoff(attempt)`, `format_duration(secs)` (`MM:SS` vs `HH:MM:SS`), `sanitize_id`/`slugify` — single source for 5+ former copy-pastes (`ytdlp`, `session`, `config`, `pipeline`, `app`).

**Slint App (`app/ui/app.slint:198` / `app/src/main.rs:354`)**: `MainWindow` router (`route` string), route owner, `preferred 980x640 min 880x560`, sidebar `210px #18181b`, bg `#121214`, panels `#1a1a1e`, fields `#202124/#3d4149`, accent `#6366f1/#0878c9`. Sidebar has single video entry: `Inicio` (covers legacy `home`+`new-project`, `app.slint:378` `route=="home"||"new-project"`), `export|settings|keys`. No recent projects list. `Inicio` contains URL input + dedicated video panel (`video-id/duration/thumbnail/thumbnail-image/has-info`, `160x90` preview) + progress + collapsible technical log (`yt-dlp/ffmpeg` via `analysis-log-lines: [string]` `VecModel` ring `400` + `spawn_thumbnail_fetch` helper). Semantic callbacks `save-settings/save-ai-settings/browse-output/fetch-info/start-analyze/...` map to Rust with no logic in `.slint`. `start-analyze` uses `session::resolve_stable_id` + `video_work_dir_or_tmp` (cache-first stable id, never `video_{timestamp}`) and `append_log_line/clear_log_lines` (`LOG_RING_CAP=400`). Translations `apply_translations()` (`locales/*.yml` `rust-i18n 4.2` → `in property <string> tr-*`). `tracing_subscriber::fmt().with_ansi(false)` (`main.rs:358`, `cli/src/main.rs:77`) and `tracing::debug!` for `config normalized` (`main.rs:377`) avoid raw ANSI `←[2m` in debug console.

**CLI (`crates/cli/src/main.rs`)**: `clap 4.6` `preview|transform|batch|analyze` (shares `core`); `analyze` now uses `session::resolve_stable_id` + `video_work_dir_or_tmp` persistent dir, not `temp/cli-*`.

## 3. Where to change what

| You need to... | Touch |
|---|---|
| Change UI/visuals | `crates/app/ui/app.slint` + `crates/app/src/main.rs` callbacks. Read `STYLE.md` first. |
| Change defaults/persistence | `crates/core/src/config/mod.rs` + `store.rs` (serde defaults, `normalize()`). Do not duplicate a key on two screens. |
| Media/ffmpeg/ytdlp | `crates/core/src/media/{ytdlp,ffmpeg,chunk,pipeline}.rs` + `crates/core/src/setup/mod.rs`. |
| AI/models | `crates/core/src/ai/{provider,mod,gemini}.rs` + `config::CustomModel`. |
| User-facing text | `locales/{en,es,ru}.yml` (all three) + `app/src/main.rs:apply_translations`. Never hardcode in `.slint`. |
| Session/DB | `crates/core/src/session/mod.rs`. |
| Plano/export | `crates/core/src/plano/{mod,schema,compiler,preview}.rs`. |
| Shared helpers | `crates/core/src/util.rs` (`fnv1a/hash_url/backoff/format_duration/sanitize/slugify`). |

Workspace layout: `Cargo.toml:3 members=["crates/core","crates/app","crates/cli"]`, `PLAN.md:57` living doc M0-M8.

## 4. How it is maintained

1. **Smallest diff:** the minimal diff that solves the request. No redesign, no token system/component library for a local fix (`STYLE.md:10`).
2. **Edit tooling:** `read` + `edit` (`write` only for new files). If `edit` fails on `\r\n` or multiple matches, read 5-10 lines of context and make `oldString` unique. **Never** `bash`/`powershell`/`python -c` with `-replace`/`sed`/`awk` on sources — hides whitespace and pollutes `git diff`.
3. **Locale:** new key goes to `en+es+ru` and `apply_translations`. No broken English fallback.
4. **Config:** new persisted field gets `#[serde(default)]`. Validate in `normalize()`, never `let _ = save()`.
5. **Checks before push:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (see `PLAN.md:204`, `STYLE.md:162`). `deny.toml`/`clippy.toml` apply.
6. **CI/CD (`.github/workflows`, `wix/`, `Cross.toml`)**: release matrix Windows exe+MSI, Linux binary+deb (`cargo-deb`), aarch64 via `cross`. `app/build.rs` embeds icon (`winresource`) + `SUBSYSTEM:WINDOWS` only in `release` (`#![cfg_attr(not(debug_assertions), windows_subsystem="windows")]` in `main.rs:1` + `cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS /ENTRY:mainCRTStartup` in `build.rs:14`) — `debug` keeps console for logs.
7. **Legacy:** v1 (`src/main.rs` ratatui/egui, `temp.json`, `Simple` encryption, `SessionState/APP_VERSION`) purged without compat — do not restore. v2 `projects/chunks/moments/exports` also purged in favor of `video_jobs/job_chunks/job_moments` (no retrocompat).

## 5. Why each decision (rationale)

- **Slint 1.17 over egui/Tauri/webview (`PLAN.md:24`)**: egui was tried in the M0 POC and dropped. Slint gives a native declarative look (`app.slint:198` fixed shell), small binary, no webview install, and `core` has no UI dep → future UI migration possible without touching business logic. `core` does not depend on Slint.
- **Flat dark square-only, no light mode (`STYLE.md:18`)**: intentional identity `#121214/#18181b/#1a1a1e`, `border-radius` forbidden, no gradients/glass/shadows, no AI-slop dashboard. A light alternative breaks contrast and adds unnecessary test/branch surface; light/dark toggle is forbidden.
- **Per-panel logs, never shared (`app.slint:313`)**: historic bug `Text { text: root.keys-status }` reused in `Add model` + drawer + `Test connection` (`app.slint:652,678,785`) made one `set_keys_status` leak `404 gemini-2.5-pro` to 3 places. Fix: `settings-status` (Settings), `keys-status` (API Keys), `models-status` (Provider/Models), `toast-msg` (global progress), `status-msg` (Inicio). Each new screen creates its own `in property <string> *-status`. Grep `keys-status` before reusing.
- **TouchArea z-order (`app.slint:624,669`)**: in Slint the last child paints on top and captures first. Putting `TouchArea{select}` *after* `HorizontalLayout{SquareButton}` killed `Delete/Select/On/Off`. Now `Rectangle { TouchArea{select} HorizontalLayout{Buttons} }` (TouchArea first, buttons last).
- **Side drawer (`app.slint:732,764`)**: each drawer (`show-key-drawer` 380px / `show-model-drawer` 400px) needs an empty `TouchArea {}` as first child to consume inner clicks, and an overlay `Rectangle{ #000000 45% TouchArea{close} }` behind it. Without that, a click on padding falls through to the overlay and closes it.
- **Lists are models, not text (`app.slint:588`, `main.rs:206,232`)**: `Text { text: root.models-list }` with `"\n"` is not selectable nor per-row actionable. Now `struct KeyRow/ModelRow {name/enabled/index}` + `in property <[KeyRow]> keys-model` / `<[ModelRow]> models-model` + `for k in keys-model: Rectangle { TouchArea{select} ... SquareButton{On/Off,Delete} }` with `VecModel` and `selected-key-index/selected-model-id`. Drawer opens with `open-key/model-drawer`, closes via overlay/Cancel.
- **Extensible CustomModel (`config/mod.rs:145`)**: `gemini-2.5-pro/flash` became obsolete (404). Research 2026-08-28 `ai.google.dev/gemini-api/docs/models` → current `gemini-3.7/3.6-flash` and `gemma-4-26b-a4b-it` all via `generativelanguage.googleapis.com/v1beta` with `base_provider="gemini"`. Hardcoding 2 strings forced a release per new model. Now `{model_id, base_provider}` lets future models work without code.
- **windows_subsystem (`app/src/main.rs:1`, `app/build.rs:14`)**: black console when launching the GUI on Windows comes from `SUBSYSTEM:CONSOLE`. Force `WINDOWS` only in `release`/`PROFILE=release` to keep `println/tracing` in `debug`.
- **Auto-install without button (`setup/mod.rs:111` + `app/src/main.rs:112`)**: req `UnboundedSyncSend` asked for "no button" — `ensure_tools_with_progress` runs on startup and in `on_fetch_info/on_start_analyze` if `!deps_ok`, with `deps-installing` guard + real `%` (`reqwest stream + zip deflate`) and `toast-msg`/`deps-error`. Prior failure: `reqwest { default-features=false }` without TLS → silent `yt-dlp.exe` download never created `%LOCALAPPDATA%/yt-shortmaker-v2/tools/yt-dlp.exe`. Fix `features=["json","multipart","stream"]`.
- **keyring (`security/mod.rs`, `app/src/main.rs:719`)**: keys stored as `keyring:"name"` references + value in OS credential store, display name auto-generated `Gemini Key N`. Do not ask the user for label/description.
- **SQLite session video-centric (`session/mod.rs:17`)**: replaces `temp.json` + `projects` list v1 and v2 `projects` table; `video_jobs|job_chunks|job_moments|video_cache` at `data_local_dir()/yt-shortmaker-v2/session.db` (bundled) + `videos/<video_id>/` work dir and `img_cache/` next to DB. No projects UI — `Inicio` is the only video entry. Resume verifies `work_dir` exists + `download_path` + each `chunk_{i}.mp4` via `ffmpeg::get_duration`, plus DB flags. Strict order: cannot analyze `i` before `0..i-1` verified.
- **i18n (`locales/*.yml:rust-i18n 4.2`) and screen responsibility (`STYLE.md:4`)**: Inicio carries URL + video preview + analysis progress (no model/chunk selector); Settings carries output/language/chunk/auto_extract/cookies/naming; AI/Keys carries provider+keys. Avoids duplicating config.
- **Config normalize (`config/mod.rs:373`)**: default `600/900` previously triggered `max_last_chunk > chunk_size, reset` on every fresh install, causing `tracing::info!("config normalized: ...")` in debug console with raw ANSI `←[2m`. Fixed to `60..=3600` range and `tracing::debug!` (`app/src/main.rs:377`), and `tracing_subscriber::fmt().with_ansi(false)` (`main.rs:358`, `cli/src/main.rs:77`) to stop stray `←[`.
- **Same-key per chunk (`ai/gemini.rs:278`)**: Gemini File API requires same `?key=` for upload/poll/generate. `analyze_with_key` holds one key for all three calls; `analyze_chunk:372` only rotates on `429/RESOURCE_EXHAUSTED` by marking disabled and re-uploading with next key. Never mixes keys for same `file_uri`.
 - **Verbose pipeline tracing (`media/pipeline.rs:78,119,161,192,253,270,292,317,351,392,415`)**: debug console only showed `chunk error` before. Now every stage logs at `INFO` (visible with default `RUST_LOG=info`) — `pipeline start`, `download check/skip/start/complete`, `split check/skip/start`, `chunk split ok`, `analyze start`, per-chunk `idx/file/start/analyzed`, `analyze_chunk start/success/failure` with model and elapsed, persist and `pipeline done`; plus `DEBUG` details for verify misses (`download verify miss`, `split verify miss`). UI `analysis-log` keeps per-panel `yt-dlp/ffmpeg` logs, terminal gets full flow without needing `RUST_LOG=debug`.
 - **Stable id + logs fluency (`ytdlp.rs:29`, `session/mod.rs:164`, `pipeline.rs:84`, `app.slint:328`, `app/src/main.rs:107`)**: Twitch `twitch.tv/videos/2859440809` previously generated `video_{timestamp}` per run -> always `is_download_verified` miss (bug "siempre desde 0"). Fix: `extract_video_id` now handles YouTube 11-char + Twitch `v{id}` + generic last-segment + `hash_url` (`url_{fnv1a}`) and never timestamp; `session::resolve_stable_id` prefers `video_cache` exact -> `video_id` variant -> `extract` -> `hash`; `video_work_dir_or_tmp` + `LOG_RING_CAP=400` + `append_log_line` + throttled `emit_ytdlp_line` (1Hz + dedup, progress 0.5%) make 697-frag low-res downloads fluent (was `+30 logs/s` freezing `invoke_from_event_loop` + `Text{wrap}` re-layout). CLI/app now share same stable id.

## 6. Hard rules for agents

### 6.1 Logs per-panel — never share
Each `set_*_status` writes to a single place. `grep keys-status` before adding a `Text`.

### 6.2 Never PowerShell/Python for small edits
`read`→`edit`/`write` always. `bash` only for `cargo check/fmt/clippy/test`, `git`, `Get-ChildItem`. If `edit` says `multiple matches`/`not found`, expand context 3-5 lines with exact indent until unique, do not switch to `-replace`.

### 6.3 TouchArea z-order / Drawers / Lists
See §5. Violate it and buttons die / drawer closes by itself / list is not interactive.

### 6.4 Change checklist (no ship without this)
- [ ] No `border-radius`, no light toggle, no card/gradient/shadow/pill/badge (`STYLE.md:8`)
- [ ] Text translated in `en+es+ru` and wired in `apply_translations`
- [ ] Config on the correct screen, no duplication
- [ ] `cargo fmt --check` / `clippy -- -D warnings` / `test --workspace` green
- [ ] `keys-status/models-status/settings-status/toast-msg` not shared
- [ ] Minimal diff, no unsolicited redesign
- [ ] No emojis, no decorative unicode, no special characters in UI — plain ASCII/text only

### 6.5 No emojis / no caracteres especiales — PROHIBIDO
**PROHIBIDO terminantemente** en `crates/app/ui/app.slint`, `crates/app/src/main.rs` (texto visible), y `locales/*.yml`: emojis (❌ ✅ ⚠️ 📂 📋 🎬 etc.), símbolos decorativos (● ◐ ▸ ▾ ── — • etc.), y cualquier unicode no-ASCII innecesario que arruine la interfaz flat dark square. Usar solo texto plano ASCII o letras acentuadas normales del idioma. Esta regla rompe la UX si se viola. Si encuentras un emoji o caracter especial en la app, eliminalo inmediatamente y reemplazalo por texto descriptivo simple (ej. "No template selected" en lugar de "❌ No template selected", ">" en lugar de "●", "-- label --" en lugar de "── label ──", "v"/">" en lugar de "▾/▸", "Working" en lugar de "◐"). On failure, the UI loses its intended minimal flat identity (`STYLE.md:8,25`).
