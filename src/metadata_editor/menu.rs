//! Menú principal para gestionar la metadata del archivo activo.

use console::style;
use std::io::{self, Write};
use std::path::Path;

use super::modification::modify_metadata_interactive;
use super::removal::remove_all_metadata;

/// Muestra el menú interactivo y delega en las acciones correspondientes.
pub fn show_edit_menu(path: &Path) -> Result<(), String> {
    loop {
        println!("\n{}", style("┌─ Opciones de Metadata ─").cyan());
        println!("{}", style("│").cyan());
        println!("{}", style("│  [1] Eliminar toda la metadata").cyan());
        println!("{}", style("│  [2] Modificar metadata específica").cyan());
        println!("{}", style("│  [3] Volver al menú principal").cyan());
        println!("{}", style("└─").cyan());

        print!("\n{}", style("│ Selecciona una opción ▸ ").cyan());
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => {
                if let Err(e) = remove_all_metadata(path) {
                    println!("\n{}", style(format!("│ Error: {}", e)).red());
                }
            }
            "2" => {
                if let Err(e) = modify_metadata_interactive(path) {
                    println!("\n{}", style(format!("│ Error: {}", e)).red());
                }
            }
            "3" => break,
            _ => {
                println!(
                    "\n{}",
                    style("│ Opción inválida. Intenta de nuevo.").yellow()
                );
            }
        }
    }

    Ok(())
}
