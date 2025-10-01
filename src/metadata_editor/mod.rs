//! Funciones para editar o eliminar metadata sensible de archivos soportados.

mod constants;
mod directory_cleanup;
mod image;
mod menu;
mod modification;
mod office;
mod removal;
mod utils;

pub use directory_cleanup::run_directory_cleanup;
pub use menu::show_edit_menu;

#[cfg(test)]
mod tests;
