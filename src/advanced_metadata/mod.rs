//! Recolección de metadata avanzada para diferentes tipos de archivo.

mod icc;
mod image;
mod archive;
mod media;
mod office;
mod odf;
mod pdf;
mod text;
mod xmp;

use crate::metadata::report::{ReportEntry, ReportSection};

pub struct AdvancedMetadataResult {
    pub section: ReportSection,
    pub risks: Vec<ReportEntry>,
}

pub use image::extract_image_metadata;
pub use archive::extract_zip_metadata;
pub use media::extract_media_metadata;
pub use office::extract_office_metadata;
pub use odf::extract_odf_metadata;
pub use pdf::extract_pdf_metadata;
pub use text::{extract_csv_metadata, extract_text_metadata};
