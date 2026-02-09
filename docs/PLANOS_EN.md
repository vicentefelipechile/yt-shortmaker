# JSON Plano (Template) Format for YT ShortMaker

"Planos" are JSON files that define the visual composition of your Shorts. They allow you to position the original video, add overlays, effects, and animated backgrounds.

## Basic Structure

A plano is an **array** of objects. Order matters: the first objects are drawn at the back, and the last ones at the front (layers).

```json
[
  { "type": "shader", ... }, // Background (Layer 0)
  { "type": "clip", ... },   // Middle (Layer 1)
  { "type": "image", ... }   // Front (Layer 2)
]
```

## Common Properties: Position and Size

Almost all objects have a `position` property with `x`, `y`, `width`, and `height`.

Values can be:
*   **Integers**: Exact pixels (e.g., `1080`, `1920`).
*   **Keywords**:
    *   `"center"` (for `x` or `y`): Centers the object.
    *   `"full"` (for `width` or `height`): Fills the available size (1080x1920).
    *   **Percentages**: Strings ending in `%` (e.g., `"50%"`).

```json
"position": {
  "x": "center",
  "y": 0,
  "width": "100%",
  "height": "50%"
}
```

## Object Types

### 1. Clip (`clip`)
Represents the source video being processed. You can use it multiple times.

*   `type`: "clip"
*   `position`: (Optional) Video position. Default: full.
*   `crop`: (Optional) Crop of the source video.
    *   Defines a crop window in pixels on the original video.
    *   `x_from`: Start pixel left (e.g., 0).
    *   `x_to`: End pixel right (e.g., 1920). If greater than `x_from`, crops width.
    *   `y_from`: Start pixel top (e.g., 0).
    *   `y_to`: End pixel bottom (e.g., 1080). If greater than `y_from`, crops height.
    *   Cropping applies **before** scaling or positioning.
*   `fit`: (Optional) Fit mode. Values: `"stretch"` (default, stretches), `"cover"` (crops), `"contain"` (letterbox).
*   `comment`: (Optional) User note.

### 2. Image (`image`)
Overlays a static image (png, jpg). Ideal for frames, logos, or watermarks.

*   `type`: "image"
*   `path`: Path to the image file (absolute or relative to json).
*   `position`: Position and size.
*   `opacity`: Opacity from 0.0 to 1.0 (Default: 1.0).

### 3. Video (`video`)
Background or overlay video (e.g., background gameplay, particle effects).

*   `type`: "video"
*   `path`: Path to the video file.
*   `position`: Position and size.
*   `loop_video`: `true` or `false`. If the video is shorter than the main clip, it loops.
*   `loop_video`: `true` or `false`. If the video is shorter than the main clip, it loops.
*   `opacity`: (Optional) Opacity from 0.0 to 1.0 (Default: 1.0).
*   `fit`: (Optional) Fit mode. Values: `"stretch"` (default), `"cover"`, `"contain"`.

### 4. Shader (`shader`)
Applies a visual effect to what is behind it. Currently supports blur.

*   `type`: "shader"
*   `effect`: Effect configuration object.
    *   `type`: "blur"
    *   `intensity`: Blur intensity (e.g., 20).
*   `position`: Area where the effect applies.

## Complete Example

```json
[
  // 1. Blurred background (Stretched original video + Blur)
  {
    "type": "clip",
    "position": { "x": 0, "y": 0, "width": "full", "height": "full" },
    "comment": "Base blurred background"
  },
  {
    "type": "shader",
    "effect": { "type": "blur", "intensity": 30 },
    "position": { "x": 0, "y": 0, "width": "full", "height": "full" }
  },

  // 2. Main video centered
  {
    "type": "clip",
    "position": { "x": "center", "y": "center", "width": "100%", "height": "auto" }
  },

  // 3. Watermark
  {
    "type": "image",
    "path": "./logo.png",
    "position": { "x": "center", "y": 1700, "width": 200, "height": "auto" },
    "opacity": 0.8
  }
]
```
