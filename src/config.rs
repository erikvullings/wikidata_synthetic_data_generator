use clap::{Arg, ArgAction, Command};
use std::fs::create_dir_all;
use std::path::Path;

use crate::processing_error::ProcessingError;

#[derive(Debug, Clone)]
pub struct Config {
    /// Preferred language for the results
    pub lang: String,
    /// Output directory, will be created automatically if it doesn't exist
    pub output_dir: String,
    /// Recreate the cache if it exists. If it doesn't exist, it will be created.
    pub recreate_cache: bool,
}

/// Get the input file and additional configuration settings
pub fn get_configuration() -> Result<(String, Config), ProcessingError> {
    let matches = Command::new("Wikidata Entity Extraction")
      .version("1.0")
      .author("Erik Vullings")
      .about("Extracts and processes Wikidata for OSINT analysis")
      .arg(Arg::new("lang")
          .short('l')
          .long("lang")
          .help("Language for labels and descriptions")
          .default_value("en"))
      .arg(Arg::new("output_dir")
          .short('o')
          .long("output")
          .help("Output directory")
          .default_value("output"))
      .arg(Arg::new("input_file")
          .help("Path to the Wikidata JSON dump")
          .required(true)
          .index(1))
      .arg(Arg::new("recreate_cache")
          .short('r')
          .long("recreate_cache")
          .help("Recreate cache from scratch: in case no cache file is found, it will also be true.")
          .action(ArgAction::SetTrue) // This makes it a flag, not requiring a value
          .default_value("false"))
      .get_matches();
    let lang = matches.get_one::<String>("lang").unwrap().to_string();
    let output_dir = matches
        .get_one::<String>("output_dir")
        .unwrap()
        .trim()
        .to_string();
    let recreate_cache = matches.get_flag("recreate_cache");
    let output_path = Path::new(&output_dir);
    if !output_path.exists() {
        create_dir_all(output_path)?;
    }
    let input_file = matches.get_one::<String>("input_file").unwrap().to_string();
    let config = Config {
        lang,
        output_dir,
        recreate_cache,
    };
    Ok((input_file, config))
}
