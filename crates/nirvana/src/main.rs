use tracing::error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod cli;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    match nirvana_core::logging::file_layer() {
        Ok(layer) => tracing_subscriber::registry().with(layer).init(),
        Err(e) => eprintln!(
            "{} logging disabled: {e}",
            console::Style::new().bold().yellow().apply_to("warning:")
        ),
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        error!("{info}");
        default_hook(info);
    }));

    let result = crate::cli::run().await;

    if let Err(e) = result {
        error!("{e:#}");
        eprintln!(
            "{} {e:#}",
            console::Style::new().bold().red().apply_to("error:")
        );
        std::process::exit(1);
    }
}
