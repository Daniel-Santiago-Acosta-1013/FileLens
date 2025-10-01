# FileLens

FileLens es una herramienta de línea de comandos escrita en Rust que permite
inspeccionar la metadata de archivos y directorios de forma interactiva. Solo
necesita que escribas la ruta o utilices la navegación integrada y te mostrará
información relevante como tamaño, propietario, fechas, hash y tipo de
contenido.

## Características

- Interfaz 100% interactiva, sin argumentos, con cabecera y tablas
  minimalistas.
- Navegación integrada con comandos `ls`, `cd`, índices numerados y `ver` para
  explorar cualquier directorio sin salir de la aplicación.
- Metadatos enriquecidos: tamaño legible, conteo de elementos de carpetas,
  propietario y grupo (Unix), permisos en octal y `rwx`, tipo MIME, hash
  SHA-256 (archivos ≤ 32 MiB) y fechas de acceso, modificación y creación.
- Detección automática del tipo de recurso: archivo, directorio o enlace
  simbólico, incluido el destino de los enlaces.
- Mensajes claros ante cualquier error de lectura u operación no permitida.

## Requisitos

- [Rust](https://www.rust-lang.org/) 1.74 o superior (el proyecto usa la edición
  2024 del lenguaje).

## Uso

```bash
# Ejecuta la aplicación en modo interactivo
cargo run
```

Comandos principales dentro de la sesión interactiva:

- `ls`: refresca el contenido del directorio actual y muestra índices.
- `cd <ruta|número>`: navega a una carpeta (acepta rutas absolutas, relativas o
  el índice del listado más reciente).
- `ver <ruta|número>` o escribir directamente una ruta/índice: muestra la
  metadata detallada.
- `..`, `salir`, `exit` o `Ctrl+D`: retroceder o finalizar según corresponda.

## Pruebas

Para verificar que el proyecto compila correctamente y que la interfaz
interactiva responde como se espera, ejecuta:

```bash
cargo check

# Ejemplo de recorrido automatizado: abrir la primera entrada y salir
printf '1\nsalir\n' | cargo run
```

## Licencia

Este proyecto se distribuye bajo la licencia MIT.
