//! Modelos compartidos para reportar metadata de manera consistente.

use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EntryLevel {
    Info,
    Warning,
    Success,
    Error,
    Muted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportEntry {
    pub label: String,
    pub value: String,
    pub level: EntryLevel,
}

impl ReportEntry {
    pub fn new(label: impl Into<String>, value: impl Into<String>, level: EntryLevel) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            level,
        }
    }

    pub fn info(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(label, value, EntryLevel::Info)
    }

    pub fn warning(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(label, value, EntryLevel::Warning)
    }

    pub fn success(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(label, value, EntryLevel::Success)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionNotice {
    pub message: String,
    pub level: EntryLevel,
}

impl SectionNotice {
    pub fn new(message: impl Into<String>, level: EntryLevel) -> Self {
        Self {
            message: message.into(),
            level,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub entries: Vec<ReportEntry>,
    pub notice: Option<SectionNotice>,
}

impl ReportSection {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
            notice: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetadataReport {
    pub system: Vec<ReportEntry>,
    pub internal: Vec<ReportSection>,
    pub risks: Vec<ReportEntry>,
    pub errors: Vec<String>,
}

impl MetadataReport {
    pub fn new() -> Self {
        Self {
            system: Vec::new(),
            internal: Vec::new(),
            risks: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MetadataOptions {
    pub include_hash: bool,
}

impl Default for MetadataOptions {
    fn default() -> Self {
        Self { include_hash: true }
    }
}
