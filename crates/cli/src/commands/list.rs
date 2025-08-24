use crate::cli::ListArgs;
use crate::discovery::TestDiscovery;
use crate::output::OutputFormatter;

pub async fn run(args: ListArgs) -> color_eyre::Result<()> {
    let (mb, pb) = OutputFormatter::create_multi_progress("Discovering tests...", None, false);
    let discovery = TestDiscovery::new()?;

    let test_files = if let Some(path) = &args.path {
        discovery.discover(path)?
    } else {
        discovery.discover_current()?
    };

    if test_files.is_empty() {
        pb.finish_with_message("No test files found.");
        return Ok(());
    }

    mb.clear()?;

    let output = OutputFormatter::format_test_files(&test_files, args.format)
        .map_err(|_| sheila::Error::generic("Failed to format test files"))?;
    print!("{}", output);

    Ok(())
}
