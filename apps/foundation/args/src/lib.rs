use std::path::PathBuf;
use std::ffi::OsString;

use color_eyre::eyre::Result;
use pico_args::Arguments;

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub config: PathBuf,
}

impl Args {
    /// Uses the `pico-args` crate to parse command line arguments.
    pub fn from_env() -> Result<Self> {
        let args = std::env::args_os().collect::<Vec<_>>();

        Ok(Self::from_vec(args)?)
    }

    fn from_vec(args: Vec<OsString>) -> Result<Self> {
        let mut args = Arguments::from_vec(args.iter().map(|s| s.into()).collect());

        let config: PathBuf = args.value_from_str("--config")?;

        Ok(Args { config })
    }
}

#[cfg(test)]
mod tests {
    use crate::Args;

    #[test]
    fn can_parse_arguments() {
        let args = vec![
            "program".into(),
            "--config".into(),
            "config.toml".into(),
        ];

        let parsed_args = Args::from_vec(args).unwrap();

        let expected = Args {
            config: "config.toml".into(),
        };

        assert_eq!(parsed_args, expected);
    }
}
