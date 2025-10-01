//! Utilidades para limpiar y modificar metadata de documentos Office basados en ZIP.

mod archive;
mod clean;
mod edit;
mod sanitize;
mod verify;
mod xml;

pub use clean::remove_office_metadata;
pub use edit::apply_office_metadata_edit;
#[cfg_attr(not(test), allow(unused_imports))]
pub use verify::verify_office_metadata_clean;

pub(crate) use archive::rewrite_docx;
pub(crate) use sanitize::{
    sanitize_app_properties, sanitize_core_properties, sanitize_custom_properties,
};
pub(crate) use xml::{app_field_spec, core_field_spec};
