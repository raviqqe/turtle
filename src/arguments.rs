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
    #[clap(long, help = "Set a log prefix")]
    pub log_prefix: Option<String>,
    #[clap(long, help = "Show no message on failure of build jobs")]
    pub quiet: bool,
    #[clap(long, help = "Show debug output", env = "TURTLE_DEBUG")]
    pub debug: bool,
    #[clap(long, help = "Show profile timings", env = "TURTLE_PROFILE")]
    pub profile: bool,
    #[clap(short, help = "Use a complementary tool")]
    pub tool: Option<Tool>,
}

pub enum Tool {
    CleanDead,
}
