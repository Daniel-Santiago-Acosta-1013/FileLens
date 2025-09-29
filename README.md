# FileLens

FileLens es una herramienta de línea de comandos escrita en Rust que permite
inspeccionar la metadata de archivos y directorios de forma interactiva. Solo
necesita que escribas la ruta del recurso que quieras analizar y te mostrará
información relevante como tamaño, fechas de modificación o permisos.

## Características

- Interfaz 100% interactiva (no recibe argumentos).
- Presentación estilizada con cabecera y tabla de resultados para una lectura
  clara y profesional.
- Detección automática del tipo de recurso: archivo, directorio o enlace
  simbólico.
- Muestra tamaños, permisos (incluyendo formato octal y `rwx` en sistemas
  tipo Unix) y fechas de acceso, modificación y creación cuando están
  disponibles.
- Manejo de errores legible cuando la ruta no existe o no puede consultarse.

## Requisitos

- [Rust](https://www.rust-lang.org/) 1.74 o superior (el proyecto usa la edición
  2024 del lenguaje).

## Uso

```bash
# Ejecuta la aplicación en modo interactivo
cargo run
```

Dentro de la aplicación, escribe la ruta del archivo o directorio que quieras
inspeccionar. Para salir puedes escribir `salir`, `exit` o presionar `Ctrl+D`.

## Pruebas

Para verificar que el proyecto compila correctamente y que la interfaz
interactiva responde como se espera, ejecuta:

```bash
cargo check

# Ejemplo de ejecución automatizada con una ruta y salir
printf 'Cargo.toml\nexit\n' | cargo run
```

## Licencia

Este proyecto se distribuye bajo la licencia MIT.
