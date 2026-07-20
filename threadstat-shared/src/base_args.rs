use std::path::PathBuf;
#[derive(clap::Parser)]
pub struct BaseArgs {
    /// Required input (one or more words)
    #[arg(required = true, num_args = 1..)]
    pub events: Vec<String>,

    #[arg(short, long, default_value = "./")]
    pub output_folder: PathBuf,
}


