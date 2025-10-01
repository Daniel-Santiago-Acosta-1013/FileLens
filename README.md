# FileLens

FileLens es una herramienta de línea de comandos escrita en Rust que permite
inspeccionar y sanear metadata de archivos **e** interaccionar con directorios
completos desde una interfaz totalmente interactiva. El flujo guía paso a paso:
solo selecciona desde el menú si deseas analizar un archivo individual o
limpiar una carpeta completa y FileLens mostrará toda la información relevante.

## Capacidades principales

- **Menú principal con historial**: selecciona entre analizar un archivo o
  ejecutar limpieza masiva. Los campos aceptan edición completa con flechas y
  recuerdan rutas usadas durante la sesión.
- **Análisis detallado de archivos**: muestra tamaño legible, permisos en
  octal y `rwx`, propietario/grupo (Unix), tipo MIME, hash SHA-256
  (archivos ≤ 32 MiB), fechas clave y destino de enlaces simbólicos.
- **Informe inteligente de directorios**: antes de limpiar, enumera todas las
  extensiones encontradas, destaca las compatibles con limpieza (imágenes y
  documentos Office) e incluye conteos exactos por categoría.
- **Limpieza masiva controlada**: permite decidir si incluir subdirectorios y
  filtrar por imágenes, documentos Office o ambos. Muestra progreso archivo por
  archivo y un resumen final de éxito/errores.
- **Edición interactiva de metadata**: tras analizar un archivo compatible se
  puede abrir un menú para eliminar metadata o modificar campos específicos de
  documentos Office.

## Requisitos

- [Rust](https://www.rust-lang.org/) 1.74 o superior (el proyecto usa la edición
  2024 del lenguaje).

## Instalación del binario

```bash
./scripts/install.sh
```

El comando compila FileLens en modo `release` y copia el binario generado a
`/usr/local/bin` (requiere permisos de escritura en esa ruta). Si prefieres otro
destino puedes personalizarlo con variables de entorno, por ejemplo:

```bash
PREFIX="$HOME/.local" ./scripts/install.sh
```

Tras la instalación asegúrate de que el directorio elegido esté en tu `PATH`.
Cuando el binario esté accesible bastará con ejecutar `filelens` desde cualquier
directorio del sistema.

## Cómo ejecutar

```bash
# Ejecuta la aplicación en modo interactivo
cargo run
```

Dentro de la sesión utiliza el menú principal para escoger la operación. En los
prompts puedes editar con las flechas izquierda/derecha y recuperar rutas
anteriores con ↑/↓ gracias al historial integrado.

## Pruebas

Para verificar que el proyecto compila correctamente y que la interfaz
interactiva responde como se espera, ejecuta:

```bash
# Validar la compilación
cargo check

# Ejecutar toda la batería de pruebas
cargo test
```

## Licencia

Este proyecto se distribuye bajo la licencia MIT.
