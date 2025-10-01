mod app;
mod directory;
mod formatting;
mod metadata;
mod ui;

use std::env;

fn main() {
    if env::args().len() > 1 {
        eprintln!(
            "FileLens es interactivo y no acepta argumentos. Ejecuta solo `cargo run` o el binario sin parámetros."
        );
        std::process::exit(1);
    }

    if let Err(error) = app::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
