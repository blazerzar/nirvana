mod cli;

fn main() {
    let result = crate::cli::run();

    if let Err(e) = result {
        eprintln!(
            "{} {e:#}",
            console::Style::new().bold().red().apply_to("error:")
        );
        std::process::exit(1);
    }
}
