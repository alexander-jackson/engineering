use std::path::PathBuf;

use color_eyre::eyre::Result;

pub struct Args {
    pub config: PathBuf,
}

impl Args {
    /// Uses the `pico-args` crate to parse command line arguments.
    pub fn from_env() -> Result<Self> {
        let mut args = pico_args::Arguments::from_env();

        let config: PathBuf = args.value_from_str("--config")?;

        Ok(Args { config })
    }
}
