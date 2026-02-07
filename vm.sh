#!/bin/bash

# Script de instalación automática para entorno de procesamiento de video
# Basado en la Guía de Configuración para Nvidia T4 y Rust

set -e # Detener el script si ocurre algún error

echo "--- Iniciando configuración del entorno ---"

# 1. Actualización del sistema e instalación de dependencias base
echo "[1/5] Actualizando sistema e instalando dependencias de C/OpenSSL..."
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev ffmpeg curl git

# 2. Instalación de yt-dlp
echo "[2/5] Instalando yt-dlp..."
sudo curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp
echo "yt-dlp instalado: $(yt-dlp --version)"

# 3. Instalación de Rust
echo "[3/5] Instalando Rust y Cargo..."
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
else
    echo "Rust ya está instalado."
fi

# 4. Configuración de persistencia y directorios
echo "[4/5] Configurando directorios de compilación y alias..."
mkdir -p $HOME/build_artifacts

# Añadir alias y variables al .bashrc si no existen
if ! grep -q "alias build-shorts" "$HOME/.bashrc"; then
    echo "" >> "$HOME/.bashrc"
    echo "# Configuración de yt-shortmaker" >> "$HOME/.bashrc"
    echo "export CARGO_TARGET_DIR=\$HOME/build_artifacts" >> "$HOME/.bashrc"
    echo "alias build-shorts='cargo build --release && mv \$CARGO_TARGET_DIR/release/yt-shortmaker ~/yt-shortmaker'" >> "$HOME/.bashrc"
    echo "Alias 'build-shorts' añadido a .bashrc"
fi

# 5. Verificación de FFmpeg NVENC
echo "[5/5] Verificando soporte NVENC en FFmpeg..."
if ffmpeg -encoders | grep -q nvenc; then
    echo "Soporte NVENC detectado correctamente."
else
    echo "ADVERTENCIA: No se detectó soporte NVENC en FFmpeg."
fi

echo "--- Configuración finalizada con éxito ---"
echo "Por favor, ejecuta 'source ~/.bashrc' para activar el alias y las variables de entorno."