# FileLens

FileLens es una aplicación de escritorio creada con Rust + Tauri + React que
permite inspeccionar y sanear metadata de archivos, además de ejecutar limpieza
masiva en directorios completos con una interfaz moderna y clara.

## Capacidades principales

- **Paneles claros por tarea**: análisis, limpieza, reportes y edición de
  metadata, todo en una sola interfaz.
- **Análisis detallado de archivos**: muestra tamaño legible, permisos en
  octal y `rwx`, propietario/grupo (Unix), tipo MIME, hash SHA-256
  (opcional en archivos ≤ 32 MiB), fechas clave y destino de enlaces simbólicos.
- **Metadata interna por formato**: EXIF + detección de XMP/IPTC en imágenes,
  diccionario Info en PDFs y propiedades core/app/custom en documentos Office.
- **Resumen de riesgos**: destaca campos sensibles (autoría, empresa, GPS, etc.).
- **Informe inteligente de directorios**: antes de limpiar, enumera todas las
  extensiones encontradas, destaca las compatibles con limpieza (imágenes y
  documentos Office) e incluye conteos exactos por categoría.
- **Limpieza masiva controlada**: permite decidir si incluir subdirectorios y
  filtrar por imágenes, documentos Office o ambos. Muestra progreso archivo por
  archivo y un resumen final de éxito/errores.
- **Edición de metadata en Office**: autor, título, asunto y empresa.

## Requisitos

- Rust (vía rustup).
- Node.js LTS y npm (solo si usarás un frontend JS).
- Dependencias de sistema para Tauri Desktop según tu sistema operativo.

### macOS (Desktop)

- Xcode o Command Line Tools (`xcode-select --install`).

## Configuración de Tauri

El archivo de configuración principal está en `src-tauri/tauri.conf.json`. Tauri
lo usa tanto en el runtime como en el CLI para definir metadatos, bundles y
comandos de build.

Si necesitas ajustar Node o Rust, los comandos recomendados por Tauri son:

```bash
# Instalar Rust con rustup (macOS/Linux)
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh

# Verificar Node y npm
node -v
npm -v
```

En este proyecto se usan los campos:

- `build.devUrl` y `build.beforeDevCommand` para el modo desarrollo (Vite).
- `build.frontendDist` y `build.beforeBuildCommand` para el bundle de producción.
- `bundle.icon` apunta a los íconos generados en `src-tauri/icons/`.

Si cambias el puerto del dev server, asegúrate de alinear `vite.config.ts` con
`build.devUrl`.

## Iconos

El ícono fuente vive en `app-icon.png`. Para regenerar el set completo:

```bash
cargo tauri icon
```

El comando requiere un PNG o SVG cuadrado con transparencia y crea los íconos
en `src-tauri/icons/`. Asegúrate de mantenerlos versionados.

## Ejecutar la interfaz gráfica (Tauri + React)

La interfaz gráfica vive en `frontend/` y usa Tauri en `src-tauri/`.

```bash
# Instala dependencias del front-end
cd frontend
npm install

# Vuelve a la raíz del proyecto
cd ..

# Inicia la app de escritorio (arranca el dev server automáticamente)
cargo tauri dev
```

Para compilar el bundle de producción:

```bash
cargo tauri build
```

Si necesitas compilar sin generar instaladores (por ejemplo, para evitar un
error de bundling), puedes usar:

```bash
cargo tauri build --no-bundle
```

Y luego generar instaladores específicos:

```bash
cargo tauri bundle -- --bundles app,dmg
```

Nota: la mayoría de plataformas requieren code signing para distribución.

## Pruebas

Para verificar que el proyecto compila correctamente y que la interfaz
interactiva responde como se espera, ejecuta:

```bash
# Validar la compilación
cargo check

# Ejecutar toda la batería de pruebas
cargo test
```

## Alcance actual y limitaciones

- **Imágenes**: se extrae EXIF y se detecta la presencia de XMP/IPTC; la
  interpretación completa de XMP/IPTC es una mejora pendiente.
- **PDF**: se lee el diccionario Info (autor, productor, fechas, etc.); XMP
  embebido aún no se analiza.
- **Office**: se leen `core.xml`, `app.xml` y `custom.xml` con parseo XML robusto.
- **Audio/video**: no hay análisis de metadata por ahora.
- **Edición de imágenes**: por ahora solo se soporta eliminación de metadata,
  no edición puntual de campos EXIF.

## Licencia

Este proyecto se distribuye bajo la licencia MIT.
