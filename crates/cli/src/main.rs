use clap::Parser;
use sheila_cli::cli::{Cli, Commands};
use sheila_cli::commands::cache::clear;
use sheila_cli::commands::control::{pause, resume, stop};
use sheila_cli::commands::{list, report, test};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Test(args) => test::run(args),
        Commands::List(args) => list::run(args).await,
        Commands::Report(args) => report::run(args).await,
        Commands::Stop(args) => stop(args).await,
        Commands::Pause(args) => pause(args).await,
        Commands::Resume(args) => resume(args).await,
        Commands::ClearCache => clear().await,
    }
}
