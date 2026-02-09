# Usage & Installation Guide - YT ShortMaker

This guide will help you install and start using YT ShortMaker to create your shorts.

## 📋 Prerequisites

Before starting, make sure you have **FFmpeg** installed on your system and added to your PATH.
*   **Windows**: [FFmpeg Installation Guide](https://phoenixnap.com/kb/ffmpeg-windows)
*   **Linux**: `sudo apt install ffmpeg`

## 🚀 Running the Application

Simply download the latest version from the "Releases" section or compile the project yourself using Cargo:

```bash
cargo run --release
```

## 🎮 User Interface (TUI)

The application uses an interactive terminal interface. You can navigate using the mouse or keyboard.

### Main Screen

1.  **Clips Directory**: Select the folder containing your original long videos.
2.  **Output Directory**: Choose where you want the generated shorts to be saved.
3.  **Select Template (Plano)**: Choose the layout design you want to apply.
    *   You can learn how to create your own templates in the **[Templates Guide](./PLANOS_EN.md)**.
4.  **Clips List**: On the right, you will see the video files found. Select one to view details.

### Controls

*   **[ Space ]**: Generate a quick preview (static frame).
*   **[ Enter ]**: Export the selected clip.
*   **[ B ]**: Batch export all clips.
*   **[ Q ]** or **[ Esc ]**: Exit the application.

## 🛠 Troubleshooting

### Exported video has a black screen at the beginning
This usually happens if the background video is not synchronized. Make sure you are using the latest version which automatically fixes timestamps.

### FFmpeg not found
Ensure that when opening a terminal (CMD or PowerShell) and typing `ffmpeg -version`, version information appears. If it says "command not found", you must add it to your environment variables.

---

⬅️ **[Back to Home](./index.md)** | 👉 **[View Templates Guide](./PLANOS_EN.md)**
