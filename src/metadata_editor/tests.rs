use super::image::{remove_image_metadata, verify_image_metadata_clean};
use super::office::{
    apply_office_metadata_edit, remove_office_metadata, verify_office_metadata_clean,
};
use super::{run_cleanup_with_sender, CleanupEvent};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tempfile::tempdir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[test]
fn remove_office_metadata_clears_docprops() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let source = dir.path().join("sample.docx");
    create_sample_docx(&source)?;

    remove_office_metadata(&source)?;

    assert!(source.exists());
    assert!(
        verify_office_metadata_clean(&source).expect("la verificación del documento limpio falló")
    );

    let mut archive = ZipArchive::new(File::open(&source)?)?;

    let mut core_contents = String::new();
    archive
        .by_name("docProps/core.xml")?
        .read_to_string(&mut core_contents)?;
    assert!(!core_contents.contains("Autor Prueba"));
    assert!(!core_contents.contains("Editor Prueba"));
    assert!(core_contents.contains("<cp:revision>1</cp:revision>"));

    let mut app_contents = String::new();
    archive
        .by_name("docProps/app.xml")?
        .read_to_string(&mut app_contents)?;
    assert!(!app_contents.contains("Microsoft Word"));
    assert!(app_contents.contains("<Pages>0</Pages>"));

    let mut custom_contents = String::new();
    archive
        .by_name("docProps/custom.xml")?
        .read_to_string(&mut custom_contents)?;
    assert!(custom_contents.trim().ends_with("docPropsVTypes\"/>"));

    Ok(())
}

#[test]
fn remove_image_metadata_strips_exif() -> Result<(), Box<dyn std::error::Error>> {
    const SAMPLE_IMAGE_WITH_EXIF: &[u8] = include_bytes!("../../tests/data/exif_sample.png");

    let dir = tempdir()?;
    let source = dir.path().join("sample.png");

    std::fs::write(&source, SAMPLE_IMAGE_WITH_EXIF)?;

    remove_image_metadata(&source)?;

    assert!(source.exists());
    assert!(
        verify_image_metadata_clean(&source).expect("la verificacion de la imagen limpia fallo"),
        "la imagen generada deberia quedar sin metadata detectable"
    );

    Ok(())
}

#[test]
fn verify_office_metadata_clean_flags_dirty_doc() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let source = dir.path().join("sample.docx");
    create_sample_docx(&source)?;

    let is_clean = verify_office_metadata_clean(&source)
        .expect("la verificación del documento original no debería fallar");
    assert!(!is_clean);

    Ok(())
}

#[test]
fn apply_office_metadata_edit_updates_author() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let source = dir.path().join("sample.docx");
    create_sample_docx(&source)?;

    apply_office_metadata_edit(&source, "dc:creator", "Nuevo Autor")
        .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

    assert!(source.exists());

    let mut archive = ZipArchive::new(File::open(&source)?)?;
    let mut core_contents = String::new();
    archive
        .by_name("docProps/core.xml")?
        .read_to_string(&mut core_contents)?;

    assert!(core_contents.contains("<dc:creator>Nuevo Autor</dc:creator>"));

    Ok(())
}

#[test]
fn cleanup_emits_progress_and_cleans_image() -> Result<(), Box<dyn std::error::Error>> {
    const SAMPLE_IMAGE_WITH_EXIF: &[u8] = include_bytes!("../../tests/data/exif_sample.png");

    let dir = tempdir()?;
    let source = dir.path().join("cleanup.png");
    std::fs::write(&source, SAMPLE_IMAGE_WITH_EXIF)?;

    let (sender, receiver) = std::sync::mpsc::channel();
    let path = source.clone();
    let handle = std::thread::spawn(move || run_cleanup_with_sender(vec![path], sender));

    let mut events = Vec::new();
    for event in receiver.iter() {
        events.push(event);
        if matches!(events.last(), Some(CleanupEvent::Finished { .. })) {
            break;
        }
    }

    handle
        .join()
        .map_err(|_| "La limpieza por lote fallo")?
        .map_err(|err| Box::<dyn std::error::Error>::from(err.to_string()))?;

    assert!(matches!(
        events.first(),
        Some(CleanupEvent::Started { total: 1 })
    ));
    assert!(events.iter().any(|event| matches!(
        event,
        CleanupEvent::Processing { index: 1, total: 1, .. }
    )));
    assert!(events.iter().any(|event| matches!(event, CleanupEvent::Success { .. })));
    assert!(events.iter().any(|event| matches!(
        event,
        CleanupEvent::Finished { successes: 1, failures: 0 }
    )));

    assert!(source.exists());
    assert!(
        verify_image_metadata_clean(&source).expect("la verificacion de la imagen limpia fallo")
    );

    Ok(())
}

fn create_sample_docx(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
    <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
    <Default Extension="xml" ContentType="application/xml"/>
    <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
    <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
    <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
    <Override PartName="/docProps/custom.xml" ContentType="application/vnd.openxmlformats-officedocument.custom-properties+xml"/>
</Types>
"#;

    const RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
    <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"#;

    const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:body>
        <w:p><w:r><w:t>Documento de prueba</w:t></w:r></w:p>
    </w:body>
</w:document>
"#;

    const CORE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/"
                   xmlns:dcmitype="http://purl.org/dc/dcmitype/"
                   xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    <dc:creator>Autor Prueba</dc:creator>
    <cp:lastModifiedBy>Editor Prueba</cp:lastModifiedBy>
    <dcterms:created xsi:type="dcterms:W3CDTF">2024-01-01T00:00:00Z</dcterms:created>
    <dcterms:modified xsi:type="dcterms:W3CDTF">2024-02-01T00:00:00Z</dcterms:modified>
    <dc:title>Documento Demo</dc:title>
    <dc:subject>Asunto Demo</dc:subject>
    <cp:revision>6</cp:revision>
</cp:coreProperties>
"#;

    const APP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
    <Application>Microsoft Word</Application>
    <Company>Compania Demo</Company>
    <Pages>2</Pages>
    <Words>345</Words>
    <Lines>12</Lines>
</Properties>
"#;

    const CUSTOM_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
    <property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="CustomField">
        <vt:lpwstr>Dato Confidencial</vt:lpwstr>
    </property>
</Properties>
"#;

    let file = File::create(path)?;
    let mut writer = ZipWriter::new(file);
    let options = FileOptions::<'_, ()>::default().compression_method(CompressionMethod::Stored);

    writer.start_file("[Content_Types].xml", options)?;
    writer.write_all(CONTENT_TYPES.as_bytes())?;

    writer.start_file("_rels/.rels", options)?;
    writer.write_all(RELS_XML.as_bytes())?;

    writer.start_file("word/document.xml", options)?;
    writer.write_all(DOCUMENT_XML.as_bytes())?;

    writer.start_file("docProps/core.xml", options)?;
    writer.write_all(CORE_XML.as_bytes())?;

    writer.start_file("docProps/app.xml", options)?;
    writer.write_all(APP_XML.as_bytes())?;

    writer.start_file("docProps/custom.xml", options)?;
    writer.write_all(CUSTOM_XML.as_bytes())?;

    writer.finish()?;

    Ok(())
}
