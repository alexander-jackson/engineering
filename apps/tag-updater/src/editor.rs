use std::fmt;
use std::path::Path;

use color_eyre::eyre::Result;

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

pub fn make_tag_edit(path: &Path, service: &str, tag: &str) -> Result<(), TagEditError> {
    let raw = std::fs::read_to_string(path)?;
    let edited = make_tag_edit_in_string(&raw, service, tag)?;

    std::fs::write(path, edited)?;

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
}
