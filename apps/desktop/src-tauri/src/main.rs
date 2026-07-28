fn main() {
    let mut args = std::env::args_os();
    let program = args.next();
    let remaining = args.collect::<Vec<_>>();

    if remaining.is_empty() || remaining.first().is_some_and(|arg| arg == "gui") {
        devknife_desktop_lib::run();
        return;
    }

    let args = program.into_iter().chain(remaining);
    if let Err(error) = devknife_cli::run_from(args) {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
