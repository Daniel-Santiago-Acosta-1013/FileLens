//! Recolección de metadata avanzada para diferentes tipos de archivo.

mod image;
mod office;
mod pdf;

pub use image::extract_image_metadata;
pub use office::extract_office_metadata;
pub use pdf::extract_pdf_metadata;
