# Guía de Uso e Instalación - YT ShortMaker

Esta guía te ayudará a instalar y comenzar a utilizar YT ShortMaker para crear tus shorts.

## 📋 Requisitos Previos

Antes de empezar, asegúrate de tener instalado **FFmpeg** en tu sistema y añadido al PATH.
*   **Windows**: [Guía de instalación de FFmpeg](https://phoenixnap.com/kb/ffmpeg-windows)
*   **Linux**: `sudo apt install ffmpeg`

## 🚀 Ejecución

Simplemente descarga la última versión desde la sección de "Releases" o compila el proyecto tú mismo con Cargo:

```bash
cargo run --release
```

## 🎮 Interfaz de Usuario (TUI)

La aplicación utiliza una interfaz de terminal interactiva. Puedes navegar usando el ratón o el teclado.

### Pantalla Principal

1.  **Directorio de Clips**: Selecciona la carpeta donde tienes tus videos originales.
2.  **Directorio de Salida**: Elige dónde quieres que se guarden los shorts generados.
3.  **Seleccionar Plano**: Elige el diseño (template) que quieres aplicar.
    *   Puedes aprender a crear tus propios planos en la **[Guía de Planos](./PLANOS_ES.md)**.
4.  **Lista de Clips**: A la derecha verás los videos encontrados. Selecciona uno para ver detalles.

### Controles

*   **[ Espacio ]**: Generar una previsualización rápida (frame estático).
*   **[ Enter ]**: Exportar el clip seleccionado.
*   **[ B ]**: Exportar todos los clips en batch (por lotes).
*   **[ Q ]** o **[ Esc ]**: Salir de la aplicación.

## 🛠 Solución de Problemas常见

### El video exportado tiene la pantalla negra al principio
Esto suele ocurrir si el video de fondo no está sincronizado. Asegúrate de estar usando la última versión que corrige los timestamps automáticamente.

### FFmpeg no se encuentra
Asegúrate de que al abrir una terminal (CMD o PowerShell) y escribir `ffmpeg -version`, aparece la información de la versión. Si dice "comando no encontrado", debes añadirlo a tus variables de entorno.

---

⬅️ **[Volver al Inicio](./index.md)** | 👉 **[Ver Guía de Planos](./PLANOS_ES.md)**
