use overnight_qt_reader::native;
fn main() {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--help" | "-h" => {
                println!(
                    "Qt reader prototype\nUsage: overnight-qt-reader [--book-json PATH] [--state-dir DIRECTORY]\nWith no book argument, reopen the last book or use the synthetic demo.\nREADER_THEME_FILE overrides the watched colors.toml without changing the desktop."
                );
                return;
            }
            "--book-json" | "--state-dir" => {
                if args.next().is_none() {
                    eprintln!("{argument} requires a path");
                    std::process::exit(2);
                }
            }
            _ => {
                eprintln!("Unknown argument. Use --help for usage.");
                std::process::exit(2);
            }
        }
    }
    // SAFETY: This is the process main thread; no Qt object exists before this sole call.
    let result = unsafe { native::launch() };
    std::process::exit(result);
}
