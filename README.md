# AutoShorts-Rust-CLI 🎬

A robust, interactive CLI tool built in Rust to automate the creation of YouTube Shorts from long-form content. It features a full Terminal User Interface (TUI), persistent settings management, automatic session recovery, and leverages Google Gemini AI for intelligent content analysis.

## ✨ Features

- **🖥️ Interactive TUI**: Full menu-based interface for easy navigation and control.
- **🤖 AI-Powered Analysis**: Uses Google Gemini to identify the best, most engaging moments.
- **⚙️ In-App Configuration**: Modify settings like output directory, GPU usage, and cookies directly from the menu.
- **🔄 Session Recovery**: Automatically resumes interrupted sessions from where you left off.
- **⚡ GPU Acceleration**: Supports NVIDIA NVENC for faster video processing.
- **🎨 Smart Composition**: Creates layered shorts with blurred backgrounds and customizable zoom/positioning.
- **🍪 Cookie Support**: Integrated support for `yt-dlp` cookies to handle age-restricted or premium content.

## 📋 Prerequisites

Before running this tool, ensure you have the following installed:

### 1. FFmpeg
```bash
# Windows (with Chocolatey)
choco install ffmpeg

# Or download from: https://ffmpeg.org/download.html
```

### 2. yt-dlp
```bash
# Windows (with pip)
pip install yt-dlp

# Or download from: https://github.com/yt-dlp/yt-dlp#installation
```

### 3. Google Gemini API Key
You will need a Google Gemini API Key to use the AI analysis features.
- Get one for free at [Google AI Studio](https://aistudio.google.com/app/apikey).
- The application will securely prompt you for this key on the first run.

## 🚀 Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/yt-shortmaker.git
cd yt-shortmaker

# Build the project
cargo build --release

# Run the application
cargo run --release
```

## 📖 Usage

### 1. First Run Setup
On the first launch, if no API key is found, you will be prompted to enter your **Google Gemini API Key**. This is saved securely to your settings.

### 2. Main Menu
The application opens to a Main Menu with three options:
- **Comenzar (Start)**: Begin the video processing workflow.
- **Configuracion (Settings)**: Adjust application settings.
- **Salir (Exit)**: Close the application.

### 3. Workflow
1.  Select **Start**.
2.  **Enter URL**: Paste the YouTube link you want to process.
3.  **Analysis**: The tool downloads and analyzes the video using AI.
4.  **Review**: Moments are detected and categorized.
5.  **Processing**: The app generates high-quality vertical shorts with your configured styling.

### 4. Settings
You can customize the following directly in the app:
- **Output Directory**: Where files are saved.
- **Auto Extract**: Automatically generate shorts after analysis.
- **GPU Acceleration**: Enable/Disable hardware encoding.
- **Shorts Style**: Adjust background opacity and main video zoom.
- **Cookies**: Path to your cookies file.

## 📁 Output Structure

```
output/
├── moments.json       # Raw JSON of identified moments
├── moments.txt        # Human-readable list
└── shorts/
    ├── short_1_funny.mp4
    ├── short_2_interesting.mp4
    └── ...
```

## ⚙️ Configuration File

Settings are stored in `settings.json`. While you can edit this file manually, it's recommended to use the **Configuracion** menu in the app.

```json
{
  "google_api_keys": ["your-api-key-here"],
  "default_output_dir": "./output",
  "gpu_acceleration": true,
  "shorts_config": {
      "background_opacity": 0.4,
      "main_video_zoom": 0.7
  }
}
```

## 🎯 Moment Categories

The AI identifies moments in these categories:
- **Funny**: Humorous or comedic moments
- **Interesting**: Educational or thought-provoking content
- **Incredible Play**: Amazing gameplay or skillful moments
- **Other**: Notable moments that don't fit above categories

## 🛠️ Development

```bash
# Run in development mode
cargo run

# Run tests
cargo test
```

## 📄 License

MIT License - feel free to use and modify as needed.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
