//! Funciones para editar o eliminar metadata sensible de archivos soportados.

pub(crate) mod constants;
mod directory_cleanup;
mod image;
mod office;
mod removal;
mod utils;

pub use directory_cleanup::{
    analyze_directory, analyze_files, collect_candidate_files, filter_files,
    run_cleanup_with_sender, CleanupEvent, DirectoryAnalysisSummary, DirectoryFilter,
};
pub use office::apply_office_metadata_edit;
pub use removal::remove_all_metadata;

#[cfg(test)]
mod tests;
