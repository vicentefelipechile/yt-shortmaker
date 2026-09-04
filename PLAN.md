# yt-shortmaker v2 - M5 Structure, Preview and Shorts Export Plan

> Status: Planned. Implementation has not started.
> Target: M5 Export and Plano editor.
> Stack: Rust 2021, Slint 1.17, Tokio, SQLite, ffmpeg and ffprobe.
> Mandatory visual rules: `STYLE.md`.

## 1. Current State

M5 is not implemented. The current repository contains the following partial pieces:

- `crates/core/src/plano/schema.rs` defines `Clip`, `Image`, `Shader { Blur }`, and `Video` objects.
- `crates/core/src/plano/compiler.rs` builds a partial ffmpeg filter graph.
- `crates/core/src/plano/preview.rs` only returns a textual layer count.
- `crates/core/src/media/pipeline.rs` logs an extraction placeholder when `auto_extract` is enabled.
- `crates/core/src/session/mod.rs` persists videos, chunks, and moments, but not structures, assets, or exports.
- `crates/app/ui/app.slint` owns the route and still contains an Export placeholder.
- `crates/app/ui/components.slint` contains the existing square controls and hit-testing patterns.
- `crates/app/src/main.rs` owns Slint callbacks, models, worker threads, and event-loop forwarding.
- The CLI preview, transform, and batch commands currently print filter information instead of rendering.

The current compiler is not sufficient for production export because it does not reliably trim each moment, map audio, verify outputs, or persist resumable export state. M5 must solve those problems before the pipeline can perform real extraction.

## 2. Confirmed Product Decisions

These decisions are fixed and must not be reinterpreted during implementation:

- Add a new global navigation entry named `Structure`.
- The Spanish translation is `Estructura`.
- Structure is a complete composition editor, not only a crop editor.
- Structure has a selector for analyzed videos.
- A new video structure starts with an empty canvas.
- The logical output canvas is 1080x1920 and uses a 9:16 ratio.
- The output frame is fixed and is not a movable layer.
- The initial Clip placement uses centered automatic cover crop.
- The user can correct that result visually and numerically.
- Layers are static for the complete short. No keyframes or animation in M5.
- Supported layers are Clip, Image, Shader Blur, and Video.
- The editor supports selection, movement, resize, visibility, deletion, and ordering.
- Local assets use an explicit file picker, not drag and drop in the first version.
- Initial asset formats are PNG, JPG, MP4, and WebM.
- Imported assets are copied into the video session.
- Structures exist as reusable global templates and per-video copies.
- Preview includes a rendered static frame and a short rendered sample video.
- Both previews use the same compiler as final export.
- Moments are selected in a table for batch export.
- M5 does not edit moment timestamps.
- Original moment audio is preserved when an audio stream exists.
- Output resolutions are 1080x1920 and 720x1280.
- The initial output codec is H.264 with AAC audio in an MP4 container.
- FPS, bitrate, and quality are selected through safe predefined presets.
- Export state is persisted per moment and is resumable.
- Empty or invalid structures cannot be automatically exported.

## 3. User Workflow

1. Open `Structure` from the sidebar.
2. Select an analyzed video.
3. Load the per-video structure, or create an empty one.
4. Add Clip, Image, Video, or Blur layers.
5. Move and resize layers on the canvas.
6. Select a layer and edit exact values in the inspector.
7. Import local assets when required.
8. Save the structure for the current video.
9. Optionally save it explicitly as the global template.
10. Choose a preview second and render a static frame.
11. Render a short sample to verify the composition and audio.
12. Select moments in the moments table.
13. Choose resolution, FPS, and quality preset.
14. Start batch export.
15. Retry failed or cancelled moments without reprocessing valid completed moments.

The UI must clearly distinguish canvas pan, canvas zoom, layer movement, output frame, preview second, and moment range.

## 4. Coordinate Contract

Persisted positions always use logical output coordinates:

```text
logical width  = 1080
logical height = 1920
```

Only the editor uses zoom:

```text
screen_x      = logical_x      * zoom
screen_y      = logical_y      * zoom
screen_width  = logical_width  * zoom
screen_height = logical_height * zoom
```

Screen coordinates must never be persisted.

The fixed output frame:

- Is always at logical `(0, 0)`.
- Has the configured output size.
- Is drawn after layers so its border remains visible.
- Has no callback, selection, or layer ID.
- Is not included in the ffmpeg graph.
- Defines the area that is exported.

When a Clip is added, Rust creates it as:

```text
x=0, y=0, width=1080, height=1920, fit=cover
```

Cover preserves aspect ratio, fills the target rectangle, crops excess pixels, and centers the source. The user can then adjust the layer.

## 5. Core Data Model

The current `Plano = Vec<PlanoObject>` is insufficient because an editor needs stable IDs, names, visibility, and output settings. Extend `crates/core/src/plano/schema.rs` with a versioned document:

```rust
pub struct PlanoDocument {
    pub version: u32,
    pub output: OutputConfig,
    pub preview_second: u64,
    pub layers: Vec<PlanoLayer>,
}

pub struct PlanoLayer {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub object: PlanoObject,
}

pub struct OutputConfig {
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub bitrate: String,
    pub fps: u32,
    pub audio_codec: String,
}
```

Every persisted field needs serde defaults where appropriate. `version` must be validated. Layer IDs, not indexes, are used in callbacks, updates, ordering operations, and persistence.

Validation must reject non-finite values, negative sizes, zero sizes, opacity outside `0.0..=1.0`, invalid fit values, and invalid asset paths.

## 6. Persistence

### 6.1 Global and per-video structures

Global template:

```text
config_dir/yt-shortmaker-v2/structures/default.json
```

Per-video structure:

```text
data_local_dir/yt-shortmaker-v2/videos/<video_id>/structure.json
```

Per-video changes must not silently overwrite the global template. Saving as the global template is an explicit action. Restoring it is also explicit.

Recommended SQLite metadata:

```sql
CREATE TABLE IF NOT EXISTS video_structures (
  video_id TEXT PRIMARY KEY,
  structure_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

The per-video JSON is the portable document. Save the JSON atomically first and then update SQLite. Any persistence failure must be shown to the user.

### 6.2 Session assets

Assets are copied to:

```text
data_local_dir/yt-shortmaker-v2/videos/<video_id>/assets/
```

Add an asset manifest:

```rust
pub struct StructureAsset {
    pub id: String,
    pub original_path: String,
    pub session_path: String,
    pub kind: AssetKind,
    pub checksum: String,
}
```

Import sequence:

1. Open the native file picker from Rust.
2. Validate extension: PNG, JPG, MP4, or WebM.
3. Verify readability and media metadata with ffprobe where applicable.
4. Generate a safe internal name.
5. Copy into the session asset directory.
6. Store checksum and copied path.
7. Create the corresponding layer.
8. Refresh the Slint model.

The compiler uses only the copied session path.

### 6.3 Export records

Add resumable state:

```sql
CREATE TABLE IF NOT EXISTS exports (
  video_id TEXT NOT NULL,
  moment_idx INTEGER NOT NULL,
  output_path TEXT NOT NULL,
  status TEXT NOT NULL,
  error TEXT,
  structure_hash TEXT NOT NULL,
  profile_hash TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  PRIMARY KEY (video_id, moment_idx)
);
```

Valid states are `pending`, `running`, `completed`, `failed`, `cancelled`, and `stale`.

A completed output is reusable only when its file exists, ffprobe opens it, and its structure and profile hashes match the current request.

## 7. Exact Slint File and Component Structure

Do not implement the whole editor in `app.slint`.

```text
crates/app/ui/
  app.slint
  components.slint
  structure-editor.slint
  structure-canvas.slint
  structure-layer-row.slint
  structure-inspector.slint
  structure-preview.slint
  structure-export.slint
```

The hierarchy is:

```text
MainWindow               inherits Window
  StructureEditor         inherits Rectangle
    StructureHeader       inherits Rectangle
    StructureCanvas       inherits Rectangle
      StructureGrid       inherits Rectangle
      CanvasLayerItem      inherits Rectangle
        ResizeHandle       inherits Rectangle
      StructureOutputFrame inherits Rectangle
    StructureLayerPanel   inherits Rectangle
      StructureLayerRow    inherits Rectangle
    StructureInspector    inherits Rectangle
      ClipInspector        inherits Rectangle
      ImageInspector       inherits Rectangle
      VideoInspector       inherits Rectangle
      ShaderInspector      inherits Rectangle
    StructurePreview      inherits Rectangle
    StructureExportPanel  inherits Rectangle
    AddLayerPanel          inherits Rectangle
```

### 7.1 `MainWindow`

`MainWindow` remains the route owner and inherits `Window`.

It owns `route`, top-level Structure properties, navigation, and callback forwarding. It must not draw layers, parse inspector values, write files, access SQLite, or run ffmpeg.

Add the `structure` route and a `NavItem` using the existing component. Keep the current sidebar width and window minimum sizes.

### 7.2 `StructureEditor`

Declaration:

```slint
export component StructureEditor inherits Rectangle
```

Properties:

```slint
in property <[StructureVideoRow]> videos-model: [];
in property <[StructureLayerRow]> layers-model: [];
in property <[CanvasLayer]> canvas-layers: [];
in property <[MomentRow]> moments-model: [];
in-out property <string> selected-video-id: "";
in-out property <string> selected-layer-id: "";
in-out property <float> zoom: 0.5;
in-out property <string> preview-second-text: "0";
in property <image> static-preview;
in property <image> sample-preview;
in property <bool> rendering-preview: false;
in property <bool> exporting: false;
in property <string> structure-status: "";
in property <string> preview-status: "";
in property <string> export-status: "";
```

Callbacks:

```slint
callback select-video(string);
callback save-structure();
callback save-global-template();
callback restore-global-template();
callback add-clip-layer();
callback add-image-layer();
callback add-video-layer();
callback add-blur-layer();
callback select-layer(string);
callback move-layer(string, float, float);
callback resize-layer(string, float, float, float, float);
callback delete-layer(string);
callback toggle-layer(string);
callback raise-layer(string);
callback lower-layer(string);
callback update-layer-name(string, string);
callback update-layer-position(string, float, float);
callback update-layer-size(string, float, float);
callback update-layer-opacity(string, float);
callback update-layer-fit(string);
callback update-layer-crop(string, int, int, int, int);
callback update-layer-video-options(string, bool, bool);
callback import-image();
callback import-video();
callback change-preview-second(int);
callback render-static-preview();
callback render-sample-preview();
callback toggle-moment(int);
callback start-export();
callback cancel-export();
callback retry-failed-export();
```

Before full implementation, compile a minimal test to verify callback parameter types. `TouchArea.mouse-x` and `mouse-y` are Slint `length` values, so the implementation must verify how they map to generated Rust types instead of assuming implicit float conversion.

The layout is a fixed-height header, a central horizontal editor area, and a bottom preview/export area. Do not wrap the complete page in one `ScrollView`.

### 7.3 `StructureHeader`

```slint
export component StructureHeader inherits Rectangle
```

Use a fixed-height square panel containing the active video summary, `Change video`, `Save structure`, `Save as global template`, and `Restore global template`. Use existing `SquareButton` components and a separate `structure-status` property.

### 7.4 Video selector

```slint
export component StructureVideoSelector inherits Rectangle
```

Properties:

```slint
in property <[StructureVideoRow]> model: [];
in-out property <bool> opened: false;
callback selected(string);
callback close();
```

Closed state shows the active video. Open state uses `ScrollView`, `VerticalLayout`, and one `Rectangle` per row. The row selection `TouchArea` must be declared before visual buttons. Never join rows into one text property.

### 7.5 `StructureCanvas`

```slint
export component StructureCanvas inherits Rectangle
```

Properties:

```slint
in property <[CanvasLayer]> layers: [];
in-out property <string> selected-layer-id: "";
in-out property <float> zoom: 0.5;
in property <int> output-width: 1080;
in property <int> output-height: 1920;
```

Callbacks:

```slint
callback select-layer(string);
callback drag-layer(string, float, float);
callback resize-layer(string, float, float, float, float);
callback change-zoom(float);
```

Internal hierarchy:

```text
StructureCanvas
  Rectangle background
    canvas toolbar
    ScrollView
      Rectangle canvas-content
        StructureGrid
        CanvasLayerItem repeated with for
        StructureOutputFrame
```

Use Slint 1.17 `ScrollView` for pan:

```slint
ScrollView {
    mouse-drag-pan-enabled: true;
    viewport-width: 1080px * root.zoom;
    viewport-height: 1920px * root.zoom;
    horizontal-scrollbar-policy: ScrollBarPolicy.as-needed;
    vertical-scrollbar-policy: ScrollBarPolicy.as-needed;
}
```

Do not add a second full-canvas `TouchArea` for panning. It would compete with layer movement.

### 7.6 Grid and output frame

`StructureGrid` inherits `Rectangle`, contains only flat one-pixel grid rectangles, and has no input callbacks.

`StructureOutputFrame` inherits `Rectangle`, has no `TouchArea`, is always at `(0, 0)`, and is drawn after all layers. It is a visual reference only and is never exported as an object.

### 7.7 `CanvasLayerItem`

```slint
export component CanvasLayerItem inherits Rectangle
```

Properties:

```slint
in property <string> layer-id;
in property <string> layer-type;
in property <float> layer-x;
in property <float> layer-y;
in property <float> layer-width;
in property <float> layer-height;
in property <bool> selected;
in property <bool> visible;
in property <float> zoom;
in property <image> preview-image;
```

Callbacks:

```slint
callback selected-layer();
callback dragged(float, float);
callback resized(float, float, float, float);
```

Geometry is always derived from logical values:

```slint
x: root.layer-x * root.zoom;

Child order is mandatory:

```text
1. Preview Rectangle or Image
2. Movement and selection TouchArea
3. Four ResizeHandle components
4. Selection border
```

The movement area must not be placed after the handles. The border must not capture input.

Slint 1.17 documents `TouchArea.moved()` without coordinate arguments. Calculate deltas from `mouse-x - pressed-x` and `mouse-y - pressed-y`, then divide by zoom. Rust applies the delta to the position captured at press time, validates it, and sends the corrected model back. The layer component must not persist a position locally.

### 7.8 `ResizeHandle`

```slint
export component ResizeHandle inherits Rectangle
```

It receives `handle` and `zoom`, uses the documented directional `MouseCursor`, and emits a proposed logical geometry. Handles are `top-left`, `top-right`, `bottom-left`, and `bottom-right`. Rust applies minimum size, bounds, aspect ratio, and layer-specific rules.

### 7.9 Layer panel

```slint
export component StructureLayerPanel inherits Rectangle
```

Use `ScrollView + VerticalLayout + for layer in model`. Rows inherit `Rectangle` and contain name, type, visibility, selection, raise, lower, and delete controls. The row selection `TouchArea` must be before buttons. All callbacks use layer IDs.

### 7.10 Inspector

```slint
export component StructureInspector inherits Rectangle
```

It receives selected-layer values and uses existing `SquareInput` controls. Position, size, opacity, and crop values are edited as strings and committed as groups. Rust parses and validates them on commit. Do not parse incomplete input on every keystroke.

Conditional components:

```slint
if root.layer-type == "clip": ClipInspector { ... }
if root.layer-type == "image": ImageInspector { ... }
if root.layer-type == "video": VideoInspector { ... }
if root.layer-type == "shader": ShaderInspector { ... }
```

Clip fields include crop and fit. Image fields include copied asset and opacity. Video fields include copied asset, loop, keep-last-frame, fit, and opacity. Shader fields include blur intensity.

### 7.11 Add-layer panel

```slint
export component AddLayerPanel inherits Rectangle
```

Actions are `Add Clip`, `Add Image`, `Add Video`, and `Add Blur`. Clip and Blur are created immediately. Image and Video invoke Rust file-picker callbacks first. Drag-and-drop is explicitly deferred until this flow is stable.

### 7.12 Preview and export panels

`StructurePreview` inherits `Rectangle` and displays `static-preview`, `sample-preview`, preview-second input, render buttons, and `preview-status`. It never runs ffmpeg.

`StructureExportPanel` inherits `Rectangle` and displays a real moment model, resolution, quality preset, FPS, per-moment status, progress, cancel, and retry actions. It must not use plain text lists.

## 8. Slint Models and Rust

Declare these UI structs:

```slint
struct StructureVideoRow {
    id: string, title: string, duration: string, moments: string,
    status: string, selected: bool,
}

struct StructureLayerRow {
    id: string, name: string, layer-type: string, visible: bool,
    selected: bool, x: float, y: float, width: float, height: float,
}

struct CanvasLayer {
    id: string, layer-type: string, x: float, y: float, width: float,
    height: float, visible: bool, selected: bool, preview-image: image,
}

struct MomentRow {
    index: int, start: string, end: string, duration: string,
    category: string, selected: bool, export-status: string,
}
```

Keep strong `Rc<VecModel<T>>` references in Rust. Use `set_row_data` for individual movement, selection, and inspector updates. Replace the whole model only when loading another video or structure.

Required Rust helpers include `refresh_structure_videos`, `refresh_structure_layers`, `refresh_structure_inspector`, `refresh_structure_moments`, `load_structure_for_video`, `save_structure_for_video`, and `set_structure_status`.

## 9. Threading and Callback Contract

Synchronous callbacks may select rows, toggle visibility, reorder in memory, or update a model row. File picking, asset copying, ffprobe, ffmpeg, preview, export, hashing, and potentially slow SQLite operations run in worker threads.

Use the existing pattern:

```rust
std::thread::spawn(move || {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(async move { /* core operation */ });
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = weak.upgrade() {
            // update UI properties or models
        }
    });
});
```

Every long operation uses a `CancellationToken`. The UI thread must never wait synchronously for media work.

Use separate statuses: `structure-status`, `preview-status`, and `export-status`. Never reuse `keys-status`, `settings-status`, or an unrelated panel status.

## 10. FFmpeg and Preview Contract

Split the Plano implementation into separate responsibilities:

```rust
pub fn validate_document(...);
pub fn compile_structure(...);
pub fn render_frame(...);
pub fn render_sample(...);
pub fn render_moment(...);
```

### 10.1 Static frame

1. Resolve active downloaded video.
2. Validate preview second against duration.
3. Validate structure and session assets.
4. Compile composition.
5. Run ffmpeg in a worker.
6. Write a temporary PNG in the video session.
7. Load it as `slint::Image`.
8. Return it using `invoke_from_event_loop`.

This does not create an export record.

### 10.2 Sample video

The sample uses the same compiler as final export, renders a documented short duration such as 3 to 5 seconds, uses a faster preview profile, writes a temporary MP4, and does not mark a moment completed. The exact sample duration and start rule must be defined in one Rust constant or configuration value.

### 10.3 Moment render

For every moment:

1. Parse start and end timestamps.
2. Require `start < end`.
3. Require at least `MIN_MOMENT_DURATION_SECONDS`.
4. Require the end within source duration.
5. Trim source video and corresponding audio.
6. Apply static layers.
7. Map the composed video explicitly.
8. Map optional original audio.
9. Use `-shortest` where appropriate.
10. Encode H.264 and AAC.
11. Render to a temporary output.
12. Verify it with ffprobe.
13. Atomically move it to the final output.

Use explicit filter labels and `-map` arguments. The official FFmpeg documentation confirms that labeled `filter_complex` outputs must be mapped and that overlay composition uses a complex filter graph. Do not invoke a shell; pass arguments directly to `Command`.

### 10.4 Layers

Clip layers use the source video, with trim and `setpts`. Image layers use copied session files, preserve alpha when needed, apply opacity, scale, and overlay. Video layers use copied files, start at zero, support loop and last-frame behavior, scale according to fit, apply opacity, and stop with the moment. Blur layers use validated `boxblur` values. The first persisted layer is the bottom layer and the last is the top layer; this convention must be used everywhere.

### 10.5 Audio and profiles

Preserve original audio when present and do not fail for videos without audio. Use optional audio mapping, AAC, H.264, `yuv420p`, and MP4.

Expose only safe presets for H.264 bitrate and quality. The preset table belongs in Rust, not duplicated in Slint. Expose resolution, quality preset, and FPS 30 or 60. Required resolutions are 1080x1920 and 720x1280.

## 11. Batch Export

Recommended output:

```text
<output_dir>/<video_slug>/shorts/short_001.mp4
<output_dir>/<video_slug>/shorts/short_002.mp4
<output_dir>/<video_slug>/moments.json
<output_dir>/<video_slug>/moments.txt
```

Before starting, require an active video, valid structure, at least one selected valid moment, valid assets, valid output directory, and valid profile. Show a summary before the worker starts.

For each moment, load or create an export record, skip only a valid matching completed output, mark `running`, render to a temporary file, verify duration/resolution/codecs, atomically finalize, and mark `completed`. On failure, save the error as `failed` and continue with other moments.

Cancellation stops new work, requests active ffmpeg termination, marks unfinished work correctly, and leaves completed outputs untouched. Retry processes failed and cancelled moments only. A structure or profile hash change makes previous outputs `stale`.

## 12. Moments and Pipeline Integration

M4 must provide the moments table. M5 requires loading `job_moments`, displaying start, end, duration, category, description when space permits, selection, and export status. Timestamps are not edited in M5.

Replace the `auto_extract` placeholder in `media/pipeline.rs`:

- `auto_extract=false`: finish after analysis persistence.
- `auto_extract=true` with a valid structure: export all valid moments through the shared export service.
- `auto_extract=true` without a structure: finish analysis and report that Structure must be configured.
- Empty structure or missing asset: block autoexport and identify the cause.
- GUI export and pipeline autoexport must use the same core service.

## 13. Exact File Changes

Core:

- `crates/core/src/plano/schema.rs`: document, IDs, output settings, defaults, validation.
- `crates/core/src/plano/compiler.rs`: temporal trim, explicit graph labels, layer order, assets, audio.
- `crates/core/src/plano/preview.rs`: frame and sample rendering.
- New `crates/core/src/plano/export.rs`: profiles, execution, progress, cancellation, verification, resumable batch.
- `crates/core/src/session/mod.rs`: structures, assets, export records, analyzed-video queries.
- `crates/core/src/media/ffmpeg.rs`: safe execution and output inspection helpers.
- `crates/core/src/media/pipeline.rs`: real shared autoexport.

App:

- `crates/app/ui/app.slint`: structure route, navigation, top-level properties, callback forwarding.
- New structure UI files defined in Section 7.
- `crates/app/src/main.rs`: translations, models, callbacks, worker operations, event-loop results.
- `locales/en.yml`, `locales/es.yml`, and `locales/ru.yml`: all user-facing strings together.

## 14. Implementation Sequence

### Phase 0: Slint smoke test

Compile a minimal test against Slint 1.17 to verify `TouchArea.moved()`, `mouse-x`, `mouse-y`, `pressed-x`, `pressed-y`, callback types, `ScrollView.mouse-drag-pan-enabled`, and explicit viewport properties. Do this before building the full editor.

### Phase 1: Static UI shell

Add the route, `StructureEditor`, static models, fixed frame, grid, and minimum-size verification. Do not connect SQLite, ffmpeg, or asset imports yet.

### Phase 2: Canvas and selection

Implement `ScrollView`, repeated `CanvasLayerItem`, stable-ID selection, layer panel, and hit-testing verification.

### Phase 3: Movement and resize

Implement one movement callback, verify logical conversion, implement one resize handle, verify its generated Rust type, then add the other handles and model updates.

### Phase 4: Inspector and layer operations

Implement inspector commit behavior, numeric validation, visibility, deletion, ordering, type-specific fields, and empty-canvas validation.

### Phase 5: Persistence

Implement versioned serialization, atomic per-video and global template files, SQLite metadata, dirty state, loading, saving, and template restore.

### Phase 6: Assets

Implement native picker callbacks, format validation, copying, manifest/checksum, image/video layer creation, and missing-asset errors.

### Phase 7: Compiler and static preview

Refactor the compiler, define ordering, implement frame rendering, preview-second validation, image loading, and filter tests.

### Phase 8: Sample preview

Implement shared-compiler short rendering, temporary files, cancellation, status, and audio/duration verification.

### Phase 9: Moments and export UI

Load analyzed videos and moments, implement selection, profiles, per-moment status, and preflight summary.

### Phase 10: Real export

Implement moment trimming, audio mapping, H.264/AAC profiles, output verification, atomic outputs, SQLite state, progress, cancellation, and retry.

### Phase 11: Pipeline integration

Replace the placeholder, reuse the export service, block invalid autoexport, forward progress, and test restart/resume.

### Phase 12: CLI and documentation

Decide which CLI commands consume `PlanoDocument`, replace mock behavior where included in M5, update help, and update this roadmap.

## 15. Testing Strategy

Core unit tests must cover document defaults, versioning, IDs, bounds, opacity, fit, crop, assets, profiles, moment duration, timestamps, fingerprints, and export state transitions.

Compiler tests must cover empty rejection, Clip, blurred background plus foreground Clip, image opacity, looping video, layer order, trimming, audio mapping, and both resolutions.

ffmpeg fixture tests must cover frame, sample, final duration, resolution, H.264, AAC, no-audio input, image/video overlays, blur, invalid assets, and invalid moments.

Manual UI QA must cover video selection, empty canvas, all layer types, selection, movement, all resize handles, inspector commits, ordering, visibility, deletion, asset import, missing assets, zoom, pan, both previews, moment selection, progress, cancellation, retry, saved reload, template restore, 880x560, 980x640, and all locales.

Required gates:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 16. Acceptance Criteria

M5 is complete only when:

- Structure is reachable from the sidebar and can select analyzed videos.
- New structures open empty and the output frame remains fixed.
- Canvas pan uses Slint `ScrollView`; zoom does not alter persisted coordinates.
- Clip, Image, Video, and Blur layers can be added, selected, moved, resized, hidden, reordered, and deleted.
- Numeric inspector commits are validated in Rust.
- PNG, JPG, MP4, and WebM assets are copied into the session.
- Per-video structures and explicit global templates work.
- Static and sample previews use the same compiler as final export.
- Moments can be selected in a real model without editing timestamps.
- Batch export preserves optional source audio and produces H.264/AAC MP4.
- 1080x1920 and 720x1280 outputs work with predefined profiles.
- Export state is resumable per moment and completed matching outputs are skipped.
- Failures and cancellation are independently retryable.
- Autoexport uses the shared service and blocks empty or invalid structures.
- Outputs are verified with ffprobe.
- All UI text is translated in English, Spanish, and Russian.
- No forbidden rounded, gradient, shadow, decorative, or light-theme UI is introduced.
- All formatting, lint, and test gates pass.

## 17. Hard Slint Rules

- `MainWindow` remains the route owner.
- Editor panels inherit `Rectangle` unless a specific Slint base type is required.
- Use `ScrollView` for canvas pan, not a competing global `TouchArea`.
- Set canvas viewport dimensions explicitly.
- Use documented `TouchArea` properties for pointer deltas.
- Do not invent `moved(x, y)` without compiling the actual API.
- Use stable IDs, never indexes, for layer operations.
- Use real models for interactive lists.
- Use `VecModel::set_row_data` for incremental updates where possible.
- Keep selection, movement, and resize interactions separate.
- Put selection areas before buttons and resize handles.
- Do not let selection borders capture input.
- Keep all persistence, validation, ffmpeg, and file operations in Rust.
- Do not parse incomplete numeric fields on every keystroke.
- Do not block the UI thread.
- Use `invoke_from_event_loop` for worker results.
- Keep Structure, Preview, and Export statuses separate.
- Do not use unrelated status properties for Structure errors.
- Do not add drag-and-drop until the explicit picker flow is stable.

## 18. Research References

The UI plan was checked against current Slint documentation and the versioned Slint 1.17 reference:

- [Slint 1.17 TouchArea](https://releases.slint.dev/1.17.0/docs/slint/reference/gestures/toucharea/): `moved()` has no coordinate arguments; `mouse-x`, `mouse-y`, `pressed-x`, `pressed-y`, pointer events, and cursors are available.
- [Slint 1.17 ScrollView](https://releases.slint.dev/1.17.0/docs/slint/reference/std-widgets/views/scrollview/): confirms `mouse-drag-pan-enabled`, viewport dimensions, viewport positions, and scroll callbacks.
- [Slint repetition and data models](https://docs.slint.dev/latest/docs/slint/guide/language/coding/repetition-and-data-models/): confirms `for item in model` and array model repetition.
- [Slint Model trait](https://docs.slint.dev/latest/docs/rust/slint/trait.Model): confirms row access and incremental model updates.
- [Slint VecModel](https://docs.rs/slint/1.17.1/slint/struct.VecModel.html): confirms mutable vector-backed models and row updates.
- [Slint 1.17 release](https://slint.dev/blog/slint-1.17-released): confirms DragArea and DropArea exist, but they are intentionally deferred.

The media plan was checked against official FFmpeg documentation:

- [FFmpeg documentation](https://ffmpeg.org/ffmpeg.html): confirms explicit `-map`, labeled `filter_complex` outputs, and overlay graph behavior.
- [FFmpeg filters documentation](https://ffmpeg.org/ffmpeg-filters.html): confirms overlay, scaling, crop, blur, and timing belong in the filter graph.

## 19. Decisions Required Before Dependent Implementation

The product scope is confirmed, but these implementation constants must be written down in Rust before their phases begin:

- Minimum layer width and height.
- Whether a layer may be entirely outside the output frame.
- Aspect-ratio behavior for each layer type during resize.
- Exact static preview filename and cleanup policy.
- Exact sample duration and start-time rule.
- Exact H.264 quality and bitrate presets.
- Exact naming-template expansion with moment index.
- Exact persisted layer-order convention.
- Exact ffmpeg termination behavior on Windows and Linux.

Each decision must have a single source of truth and a test before the dependent feature is implemented.
