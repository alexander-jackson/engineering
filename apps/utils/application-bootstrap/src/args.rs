use clap::{Parser, ValueEnum};

/// The type of application to create.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ApplicationType {
    Rust,
}

/// The arguments for the application.
#[derive(Debug, Parser)]
pub struct Args {
    /// The name of the application to create.
    #[clap(short, long)]
    pub name: String,
    /// The type of application to create.
    #[clap(short, long, value_enum, default_value_t = ApplicationType::Rust)]
    pub application_type: ApplicationType,
}
