use clap::Parser;

#[derive(Parser)]
#[clap(about = "The Ninja build system clone written in Rust", version)]
pub struct Arguments {
    #[clap(short, help = "Set a root build file")]
    pub file: Option<String>,
    #[clap(short = 'C', help = "Set a working directory")]
    pub directory: Option<String>,
    #[clap(short, help = "Set a job limit")]
    pub job_limit: Option<usize>,
    #[clap(long, help = "Show debug output")]
    pub debug: bool,
}
