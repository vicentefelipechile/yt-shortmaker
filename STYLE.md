# yt-shortmaker UI and Project Style

> This file is mandatory guidance for every future change to the project. It exists to prevent
> unsolicited redesigns, generic AI-generated interfaces, and regressions to the established UI.

## 1. Source of Truth

- Read `STYLE.md` before modifying `crates/app/ui/app.slint` or any UI callback.
- Read `PLAN.md` before changing project architecture, configuration ownership, or workflows.
- Preserve the current visual language and component structure unless the user explicitly requests a redesign.
- Prefer the smallest change that solves the requested problem.
- Never replace an existing interface with a generic design system, dashboard, or template.

## 2. Visual Identity

The application has one intentional visual language: flat, dark, compact, and square.

- The application is dark-only. Do not add a light theme, theme selector, or light/dark toggle.
- All panels, fields, buttons, navigation items, cards, and status areas must have square corners.
- `border-radius` is prohibited in the UI.
- Gradients, glassmorphism, blur effects in the UI, drop shadows, decorative patterns, and translucent cards are prohibited.
- Do not introduce the typical AI-slop purple/blue gradient palette.
- Existing accent colors are allowed only when they match the established palette and purpose.
- Do not replace the current layout with large hero sections, oversized typography, metric cards, or excessive whitespace.
- Do not add emojis or decorative icons to make a screen appear more complete.
- Keep text sufficiently bright for readability on the dark background.

### Established Palette

Use the existing palette before introducing a new color:

| Purpose | Color |
|---|---|
| Main background | `#121214` |
| Sidebar | `#18181b` |
| Existing panel | `#1a1a1e` |
| Field background | `#202124` |
| Separator | `#27272a` |
| Field border | `#3d4149` |
| Primary action | `#0878c9` |
| Existing navigation accent | `#6366f1` |
| Primary text | `#ffffff` / `#f4f4f5` |
| Secondary text | `#d4d4d8` / `#c3c6cc` |
| Muted text | `#a1a1aa` / `#858b96` |

## 3. Layout Rules

- Preserve the compact desktop layout: approximately `980x640`, minimum `880x560`.
- Preserve the `210px` left navigation panel.
- Preserve the simple content layout with a `24px` content inset and restrained spacing.
- Use horizontal separators to divide sections instead of rounded cards or decorative containers.
- Existing square boxes are intentional. Do not remove them to make the UI look more modern.
- Do not wrap every field in a new card or panel.
- A new panel is justified only when it represents a real existing section or improves hierarchy without changing the layout language.
- Keep controls aligned to a common left edge and avoid arbitrary widths.
- Do not make a control larger merely to fill empty space.

## 4. Screen Responsibilities

Each screen has a clear responsibility. Do not move controls between screens for convenience.

### Home

- Shows recent projects and the empty state.
- Contains the existing action to create a new project.
- Must not contain unrelated processing or provider settings.

### New Project

- Accepts the source URL.
- Fetches source metadata.
- Starts the analysis process.
- Shows analysis progress and status.
- Must not contain the global AI model selector.
- Must not contain chunk configuration.
- Must not contain API key entry.
- Must not contain export settings.
- Manual moment editing belongs to Review, not this screen.

### Settings

- Contains global application defaults: output directory, language, processing defaults, cookies, and output naming.
- Chunk duration is configured here in minutes and stored internally in seconds.
- Must not contain provider-specific model selection or API key fields.
- Must not contain a light/dark theme option.

### AI / Keys

- Contains the active AI provider configuration.
- Contains the model selection used globally by analysis.
- Contains the API key input and configured key list.
- Adding a key requires only the key value.
- Key display names are generated automatically by the application.
- Do not require users to invent labels, descriptions, aliases, or metadata before saving a key.

### Export

- Contains standalone export controls and Plano workflow.
- Must not duplicate source analysis settings.

## 5. Slint Construction

- Keep `MainWindow` as the route owner and preserve its existing route names.
- Create a reusable component only when it enforces an existing visual rule, such as a square button or square input.
- Reusable components must remain visually simple and predictable.
- Do not introduce a component library, theme abstraction, design tokens system, or layout framework for a small local change.
- Avoid default widgets when their platform styling violates the dark square visual language; use a minimal custom control instead.
- Custom controls must preserve keyboard focus, text editing, and accessibility behavior.
- Use `TextInput` only when its appearance can be made readable against the project background.
- Keep callbacks semantic: `save-settings`, `save-ai-settings`, `browse-output`, and similar names must describe the action.
- Do not put business logic, file operations, or persistence inside `.slint` files.
- All persistence and validation belongs in Rust.
- UI state must not silently overwrite configuration that belongs to another screen.

## 6. Text and Localization

- UI text must not be hardcoded in `.slint` when it is user-facing.
- Add translations to `locales/en.yml`, `locales/es.yml`, and `locales/ru.yml` together.
- Use concise labels that describe the actual action or setting.
- Do not use vague labels such as `Continue`, `Configure`, or `Manage` when a specific label is possible.
- Do not expose implementation terminology to users unless it is an established product term, such as `yt-dlp`, `Gemini`, or `Plano`.

## 7. Configuration Ownership

- Global processing configuration belongs in `AppConfig.processing`.
- Provider model configuration belongs in `AppConfig.ai.providers` and is edited in `AI / Keys`.
- Cookies configuration belongs in `AppConfig.cookies` and is edited in Settings.
- Output configuration belongs in `AppConfig.export` and is edited in Settings or Export when specifically required.
- Do not duplicate one configuration value across multiple screens or config sections.
- Use serde defaults for new persisted fields.
- Validate and normalize user input before saving.
- Never silently discard a persistence error; show it in the UI and log it where appropriate.

## 8. Explicitly Forbidden AI Slop

The following are prohibited unless the user explicitly requests them:

- Rounded cards, rounded buttons, rounded inputs, pills, badges, or floating panels.
- Purple/blue gradients or arbitrary modern SaaS palettes.
- Glassmorphism, shadows, neon borders, glow effects, and decorative background blobs.
- Hero headers, oversized page titles, dashboard metric cards, onboarding carousels, or fake activity panels.
- Duplicating settings on multiple screens.
- Adding controls because an empty area exists.
- Asking for unnecessary names, labels, descriptions, confirmations, or setup steps.
- Replacing functional controls with icon-only controls without a clear tooltip and keyboard path.
- Adding light mode because it is common in template interfaces.
- Changing the navigation, sizing, spacing, typography, or visual hierarchy without a concrete request.
- Rewriting an entire screen when a local style correction is sufficient.

## 9. Change Checklist

Before submitting a UI change, verify:

- [ ] The requested behavior is implemented without moving unrelated controls.
- [ ] No `border-radius` was added.
- [ ] No light/dark selector or alternate theme was added.
- [ ] No AI-slop card, gradient, shadow, pill, badge, or decorative element was added.
- [ ] Existing square boxes and separators remain present.
- [ ] Text is readable on the dark background.
- [ ] Configuration controls are on the correct screen.
- [ ] API key entry asks only for the API key.
- [ ] User-facing text is translated in all three locale files.
- [ ] `cargo fmt --check` passes.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo test --workspace` passes.
- [ ] The final diff is limited to the requested change.
