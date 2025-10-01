//! Funciones para editar o eliminar metadata sensible de archivos soportados.

mod constants;
mod image;
mod menu;
mod modification;
mod office;
mod removal;
mod utils;

pub use menu::show_edit_menu;

#[cfg(test)]
mod tests;
