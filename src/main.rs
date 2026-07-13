use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("update") => run_update(),
        Some(argument) => {
            eprintln!("Unknown command: {argument}\nUsage: bingwall [update]");
            ExitCode::FAILURE
        }
        None => match bingwall::app::run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Could not start Bingwall: {error}");
                ExitCode::FAILURE
            }
        },
    }
}

fn run_update() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("Could not start the updater: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(bingwall::service::run_scheduled_update()) {
        Ok(path) => {
            println!("Applied {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            bingwall::service::mark_failed_update(&error);
            eprintln!("Wallpaper update failed: {error}");
            ExitCode::FAILURE
        }
    }
}
