use std::path::Path;

use color_eyre::eyre::{Result, eyre};

pub fn make_tag_edit(path: &Path, service: &str, tag: &str) -> Result<()> {
    let raw = std::fs::read_to_string(path)?;
    let edited = make_tag_edit_in_string(&raw, service, tag)?;

    std::fs::write(path, edited)?;

    Ok(())
}

fn make_tag_edit_in_string(raw: &str, service: &str, tag: &str) -> Result<String> {
    let mut lines: Vec<_> = raw.lines().map(ToString::to_string).collect();
    let services = lines
        .iter()
        .position(|line| line == "services:")
        .ok_or_else(|| eyre!("Failed to find services block"))?;

    let specific_service = lines
        .iter()
        .skip(services)
        .position(|line| line == &format!("  {service}:"))
        .ok_or_else(|| eyre!("Failed to find block for {service}"))?;

    let tag_line = lines
        .iter()
        .skip(services + specific_service)
        .position(|line| line.starts_with("    tag:"))
        .ok_or_else(|| eyre!("Failed to find tag key for {service}"))?;

    let line = &mut lines[services + specific_service + tag_line];
    *line = format!("    tag: {tag}");

    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::Result;

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
}
