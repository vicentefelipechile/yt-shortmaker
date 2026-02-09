# Formato de Planos (Templates) JSON para YT ShortMaker

Los "Planos" (Plantillas) son archivos JSON que definen la estructura visual de tus Shorts. Te permiten posicionar el video original, añadir superposiciones, efectos y fondos animados.

> [!IMPORTANT]
> **Rendimiento:** La generación de Shorts con composición (capas, filtros, efectos) requiere recodificar el video, por lo que este proceso **tarda mucho más tiempo** que la simple extracción de clips. Por favor, ten paciencia durante la exportación.

## Estructura Básica

Un plano es una **lista (array)** de objetos. El orden es importante: los primeros objetos se dibujan al fondo y los últimos al frente (capas).

```json
[
  { "type": "shader", ... }, // Fondo (Capa 0)
  { "type": "clip", ... },   // En medio (Capa 1)
  { "type": "image", ... }   // Frente (Capa 2)
]
```

## Propiedades Comunes: Posición y Tamaño

Casi todos los objetos tienen una propiedad `position` con `x`, `y`, `width` y `height`.

Los valores pueden ser:
*   **Números enteros**: Píxeles exactos (ej: `1080`, `1920`).
*   **Palabras clave**:
    *   `"center"` (para `x` o `y`): Centra el objeto.
    *   `"full"` (para `width` o `height`): Ocupa todo el tamaño disponible (1080x1920).
    *   **Porcentajes**: Strings terminados en `%` (ej: `"50%"`).

```json
"position": {
  "x": "center",
  "y": 0,
  "width": "100%",
  "height": "50%"
}
```

## Tipos de Objetos

### 1. Clip (`clip`)
Representa el video original que estás procesando. Puedes usarlo varias veces.

*   `type`: "clip"
*   `position`: (Opcional) Posición del video. Default: full.
*   `crop`: (Opcional) Recorte del video fuente original.
    *   Define una ventana de recorte en píxeles sobre el video original.
    *   `x_from`: Píxel inicial izquierda (ej: 0).
    *   `x_to`: Píxel final derecha (ej: 1920). Si es mayor que `x_from`, recorta el ancho.
    *   `y_from`: Píxel inicial arriba (ej: 0).
    *   `y_to`: Píxel final abajo (ej: 1080). Si es mayor que `y_from`, recorta la altura.
    *   El recorte se aplica **antes** de escalar o posicionar el clip.
    *   **Ejemplo:** Para recortar un video 1920x1080 y dejar solo un cuadrado central de 1080x1080:
        ```json
        "crop": {
          "x_from": 420,  // (1920 - 1080) / 2 = 420
          "x_to": 1500    // 420 + 1080 = 1500
        }
        ```
*   `fit`: (Opcional) Modo de ajuste. Valores: `"stretch"` (default, estira), `"cover"` (recorta), `"contain"` (bandas negras).
*   `comment`: (Opcional) Nota para el usuario.

### 2. Imagen (`image`)
Superpone una imagen estática (png, jpg). Ideal para marcos, logos o marcas de agua.

*   `type`: "image"
*   `path`: Ruta al archivo de imagen (absoluta o relativa al json).
*   `position`: Posición y tamaño.
*   `opacity`: Opacidad de 0.0 a 1.0 (Default: 1.0).

### 3. Video (`video`)
Video de fondo o superpuesto (ej: gameplay de fondo, efectos de partículas).

*   `type`: "video"
*   `path`: Ruta al archivo de video.
*   `position`: Posición y tamaño.
*   `loop_video`: `true` o `false`. Si el video es más corto que el clip, se repite en bucle.
*   `opacity`: (Opcional) Opacidad de 0.0 a 1.0 (Default: 1.0).
*   `fit`: (Opcional) Modo de ajuste. Valores: `"stretch"` (default), `"cover"`, `"contain"`.

### 4. Shader (`shader`)
Aplica un efecto visual a lo que hay detrás (o genera un fondo). Actualmente soporta desenfoque (blur).

*   `type`: "shader"
*   `effect`: Objeto con la configuración del efecto.
    *   `type`: "blur"
    *   `intensity`: Intensidad del desenfoque (ej: 20).
*   `position`: Área donde aplicar el efecto.

## Ejemplo Completo

```json
[
  // 1. Fondo borroso (Video original estirado y desenfocado)
  {
    "type": "clip",
    "position": { "x": 0, "y": 0, "width": "full", "height": "full" },
    "comment": "Fondo borroso base"
  },
  {
    "type": "shader",
    "effect": { "type": "blur", "intensity": 30 },
    "position": { "x": 0, "y": 0, "width": "full", "height": "full" }
  },

  // 2. Video principal centrado
  {
    "type": "clip",
    "position": { "x": "center", "y": "center", "width": "100%", "height": "auto" }
  },

  // 3. Marca de agua
  {
    "type": "image",
    "path": "./logo.png",
    "position": { "x": "center", "y": 1700, "width": 200, "height": "auto" },
    "opacity": 0.8
  }
]
```

## Ejemplos y Casos de Uso Comunes

A continuación se presentan varios ejemplos prácticos. Copia y pega el código JSON en tu archivo `.json` de plano.

### 1. Fondo Borroso Simple (Default)
El video original se usa de fondo (estirado y borroso) y también como elemento principal en el centro.

![Ejemplo Fondo Borroso](./images/example_blur.png)

```json
[
  {
    "type": "clip",
    "position": {
      "width": "full",
      "height": "full"
    },
    "fit": "stretch",
    "comment": "Fondo estirado"
  },
  {
    "type": "shader",
    "effect": {
      "type": "blur",
      "intensity": 20
    },
    "position": {
      "width": "full",
      "height": "full"
    }
  },
  {
    "type": "clip",
    "position": {
      "x": "center",
      "y": "center",
      "width": "100%",
      "height": "40%"
    },
    "comment": "Video principal"
  }
]
```

### 2. Fondo de Gameplay (Video Externo)
Un video de "gameplay" (ej: Minecraft, GTA) se reproduce en bucle como fondo, y el video original aparece centrado.

![Ejemplo Gameplay](./images/example_gameplay.png)

```json
[
  {
    "type": "video",
    "path": "./media/gameplay_background.mp4",
    "position": { "width": "full", "height": "full" },
    "loop_video": true,
    "fit": "cover",
    "opacity": 1.0,
    "comment": "Video de fondo en bucle"
  },
  {
    "type": "clip",
    "position": { "x": "center", "y": "center", "width": "100%", "height": "40%" },
    "comment": "Clip principal encima del gameplay"
  }
]
```

### 3. Pantalla Dividida (Split Screen)
Dos videos apilados verticalmente. Útil para comparaciones o videoreacciones.
(Aquí usamos el mismo clip dos veces, pero podrías usar `video` para el segundo).

![Ejemplo Split Screen](./images/example_split.png)

```json
[
  {
    "type": "clip",
    "position": {
      "x": 0,
      "y": 0,
      "height": "50%"
    },
    "crop": {
      "x_from": 420,
      "x_to": 1500
    },
    "comment": "Parte superior"
  },
  {
    "type": "clip",
    "position": {
      "x": 0,
      "y": "50%",
      "width": "100%",
      "height": "50%"
    },
    "crop": {
      "x_from": 1300,
      "x_to": 1920,
      "y_from": 500,
      "y_to": 1080
    },
    "comment": "Parte inferior"
  }
]
```

### 4. Marco / Overlay
Video con una imagen PNG transparente superpuesta (marco, estadísticas, branding).

![Ejemplo Overlay](./images/example_overlay.png)

```json
[
  {
    "type": "clip",
    "position": { "width": "full", "height": "full" },
    "fit": "cover"
  },
  {
    "type": "image",
    "path": "./media/marco_overlay.png",
    "position": { "x": 0, "y": 0, "width": "full", "height": "full" },
    "opacity": 1.0,
    "comment": "Imagen PNG con transparencia"
  }
]
```

