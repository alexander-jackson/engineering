mod args;

use std::{path::Path, process::Command};

use clap::Parser;
use color_eyre::eyre::{Result, eyre};
use git2::Repository;

use crate::args::Args;

fn setup() -> Result<()> {
    color_eyre::install()?;
    foundation_logging::install_default_registry()?;

    Ok(())
}

fn main() -> Result<()> {
    setup()?;

    let args = Args::parse();
    tracing::debug!(?args, "parsed some command line arguments");

    // find the root of the repository we are in
    let repository = Repository::discover(".")?;
    let root = repository.workdir().unwrap();
    tracing::debug!(?root, "found the root of the repository");

    // copy over the actions files
    copy_and_replace_actions_files(root, &args.name)?;

    // create the new application directory
    let application_directory = root.join("apps");

    let command = Command::new("cargo")
        .arg("new")
        .arg(&args.name)
        .current_dir(application_directory)
        .status()?;

    if !command.success() {
        return Err(eyre!("failed to add new application to the project"));
    }

    Ok(())
}

fn copy_and_replace_file(root: &Path, filename: &str, application: &str) -> Result<()> {
    let path = root.join(filename);
    let content = std::fs::read_to_string(&path)?.replace("<application>", application);

    let target = root.join(
        filename
            .replace(".template", "")
            .replace("application", application),
    );

    std::fs::write(&target, content)?;

    tracing::info!(?application, ?target, "copied and replaced file");

    Ok(())
}

fn copy_and_replace_actions_files(root: &Path, application: &str) -> Result<()> {
    let workflow_directory = root.join(".github").join("workflows");

    copy_and_replace_file(
        &workflow_directory,
        "application-ci.yaml.template",
        application,
    )?;

    copy_and_replace_file(
        &workflow_directory,
        "application-release.yaml.template",
        application,
    )?;

    Ok(())
}
