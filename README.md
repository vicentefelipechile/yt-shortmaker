# AutoShorts-Rust-CLI 🎬

A robust CLI tool built in Rust to automate the creation of YouTube Shorts from long-form content. It features a persistent settings system, an interactive real-time dashboard with uptime tracking, and leverages Google Gemini AI for intelligent content analysis.

## ✨ Features

- **AI-Powered Analysis**: Uses Google Gemini to identify the best moments for shorts
- **Automatic Chunking**: Smart video splitting (30-min chunks, 45-min buffer)
- **Interactive Dashboard**: Real-time progress with uptime tracking
- **Persistent Settings**: Configuration saved in `settings.json`
- **High-Quality Output**: Downloads source video in best quality for final clips
- **Category Detection**: Automatically categorizes moments (Funny, Interesting, Incredible Play, etc.)

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
1. Go to [Google AI Studio](https://aistudio.google.com/app/apikey)
2. Create a new API key
3. You'll be prompted to enter this key on first run

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

1. **First Run**: On first launch, you'll be prompted to configure:
   - Google Gemini API Key
   - Default output directory

2. **Enter YouTube URL**: Paste the URL of the video you want to process

3. **Wait for Analysis**: The tool will:
   - Download a low-res version for analysis
   - Split into chunks
   - Analyze each chunk with Gemini AI

4. **Review Moments**: Identified moments are saved to `moments.json` and `moments.txt`

5. **Generate Shorts**: Confirm to download high-res and extract clips

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

## ⚙️ Configuration

Settings are stored in `settings.json`:

```json
{
  "google_api_key": "your-api-key-here",
  "default_output_dir": "./output"
}
```

## 🎯 Moment Categories

The AI identifies moments in these categories:
- **Funny**: Humorous or comedic moments
- **Interesting**: Educational or thought-provoking content
- **Incredible Play**: Amazing gameplay or skillful moments
- **Other**: Notable moments that don't fit above categories

## 📊 Technical Details

- **Chunk Size**: 30 minutes (with 45-min last chunk buffer)
- **Short Duration**: 10-90 seconds (as recommended for YouTube Shorts)
- **Output Format**: MP4 (H.264 video, AAC audio)
- **AI Model**: Gemini 3 Pro Preview

## 🛠️ Development

```bash
# Run in development mode
cargo run

# Run tests
cargo test

# Build for release
cargo build --release
```

## 📄 License

MIT License - feel free to use and modify as needed.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
