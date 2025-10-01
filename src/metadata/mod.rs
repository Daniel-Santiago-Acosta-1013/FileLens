//! Consulta y despliegue de metadata básica y avanzada del sistema de archivos.

mod hashing;
mod mime;
mod output;
mod permissions;
mod renderer;

pub use renderer::render_metadata;
