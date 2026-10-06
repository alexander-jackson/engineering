use std::fmt;
use std::path::Path;

use color_eyre::eyre::Result;
use serde::Serialize;

#[derive(Debug)]
pub enum TagEditError {
    Io(std::io::Error),
    Raw(RawTagEditError),
}

impl From<std::io::Error> for TagEditError {
    fn from(value: std::io::Error) -> Self {
        TagEditError::Io(value)
    }
}

impl From<RawTagEditError> for TagEditError {
    fn from(value: RawTagEditError) -> Self {
        TagEditError::Raw(value)
    }
}

impl fmt::Display for TagEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagEditError::Io(e) => write!(f, "I/O error: {}", e),
            TagEditError::Raw(e) => write!(f, "Tag edit error: {}", e),
        }
    }
}

impl std::error::Error for TagEditError {}

/// Applies a set of tag edits to a file, returning which services were updated and skipped.
///
/// Services missing from the file are skipped rather than treated as errors, but any other
/// failure means nothing is written.
pub fn make_tag_edits<'a>(
    path: &Path,
    edits: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<TagEditOutcome, TagEditError> {
    let mut contents = std::fs::read_to_string(path)?;
    let mut outcome = TagEditOutcome::default();

    for (service, tag) in edits {
        match make_tag_edit_in_string(&contents, service, tag) {
            Ok(edited) => {
                contents = edited;
                outcome.updated.push(service.to_string());
            }
            Err(RawTagEditError::ServiceNotFound(_)) => {
                tracing::warn!(%service, "service not found in the file, skipping");
                outcome.skipped.push(service.to_string());
            }
            Err(e) => return Err(e.into()),
        }
    }

    if !outcome.updated.is_empty() {
        std::fs::write(path, contents)?;
    }

    Ok(outcome)
}

#[derive(Debug, Default, Serialize)]
pub struct TagEditOutcome {
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum RawTagEditError {
    ServicesBlockNotFound,
    ServiceNotFound(String),
    TagKeyNotFound(String),
}

impl fmt::Display for RawTagEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RawTagEditError::ServicesBlockNotFound => {
                write!(f, "'services:' block not found in the file")
            }
            RawTagEditError::ServiceNotFound(service) => {
                write!(f, "Service '{}' not found in the file", service)
            }
            RawTagEditError::TagKeyNotFound(service) => {
                write!(f, "'tag:' key not found for service '{}'", service)
            }
        }
    }
}

impl std::error::Error for RawTagEditError {}

fn make_tag_edit_in_string(raw: &str, service: &str, tag: &str) -> Result<String, RawTagEditError> {
    let mut lines: Vec<_> = raw.lines().map(ToString::to_string).collect();
    let services = lines
        .iter()
        .position(|line| line == "services:")
        .ok_or(RawTagEditError::ServicesBlockNotFound)?;

    let specific_service = lines
        .iter()
        .skip(services)
        .position(|line| line == &format!("  {service}:"))
        .ok_or_else(|| RawTagEditError::ServiceNotFound(service.to_string()))?;

    let tag_line = lines
        .iter()
        .skip(services + specific_service)
        .position(|line| line.starts_with("    tag:"))
        .ok_or_else(|| RawTagEditError::TagKeyNotFound(service.to_string()))?;

    let line = &mut lines[services + specific_service + tag_line];
    *line = format!("    tag: {tag}");

    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;

    use crate::editor::RawTagEditError;

    use super::make_tag_edit_in_string;

    #[test]
    fn can_edit_basic_file() -> Result<()> {
        let before = std::fs::read_to_string("resources/before/simple.yaml")?;
        let after = std::fs::read_to_string("resources/after/simple.yaml")?;

        assert_eq!(
            make_tag_edit_in_string(&before, "frontend", "20230614-1830")?,
            after
        );

        Ok(())
    }

    #[test]
    fn can_edit_with_multiple_services_in_file() -> Result<()> {
        let before = std::fs::read_to_string("resources/before/multiple-services.yaml")?;
        let after = std::fs::read_to_string("resources/after/multiple-services.yaml")?;

        assert_eq!(
            make_tag_edit_in_string(&before, "frontend", "20230614-1830")?,
            after
        );

        Ok(())
    }

    #[test]
    fn returns_error_if_service_not_found() -> Result<()> {
        let before = std::fs::read_to_string("resources/before/simple.yaml")?;

        let result = make_tag_edit_in_string(&before, "does-not-exist", "20230614-1830");

        let expected = RawTagEditError::ServiceNotFound("does-not-exist".to_string());

        assert!(result.is_err_and(|e| e == expected));

        Ok(())
    }

    #[test]
    fn skips_missing_services_and_applies_the_rest() -> Result<()> {
        let before = std::fs::read_to_string("resources/before/simple.yaml")?;
        let after = std::fs::read_to_string("resources/after/simple.yaml")?;

        let path = std::env::temp_dir().join("tag-updater-skip-test.yaml");
        std::fs::write(&path, before)?;

        let edits = [
            ("does-not-exist", "20230614-1830"),
            ("frontend", "20230614-1830"),
        ];
        let outcome = super::make_tag_edits(&path, edits)?;

        assert_eq!(outcome.updated, ["frontend"]);
        assert_eq!(outcome.skipped, ["does-not-exist"]);
        assert_eq!(std::fs::read_to_string(&path)?, after);

        std::fs::remove_file(&path)?;

        Ok(())
    }
}
