//! The headless MovieBox command line interface.
//!
//! The terminal UI is for people; this binary is for scripts. It shares the
//! provider engine with the app, so it works without a terminal, in a
//! container or from a cron job.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let options = match moviebox_tui::cli::parse_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    match moviebox_tui::cli::run(&options).await {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
