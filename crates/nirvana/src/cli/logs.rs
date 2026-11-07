use console::{Term, style};
use dialoguer::Confirm;
use nirvana_core::Nirvana;
use tracing::info;

use crate::cli::theme;

pub(crate) async fn view(lines: Option<usize>, follow: bool) -> anyhow::Result<()> {
    let mut reader = Nirvana::open_logs().await?;
    let term = Term::stdout();

    for line in reader.tail(lines).await? {
        term.write_line(&line)?;
    }
    if !follow {
        return Ok(());
    }
    loop {
        for line in reader.follow().await? {
            term.write_line(&line)?;
        }
    }
}

pub(crate) async fn clear(yes: bool) -> anyhow::Result<()> {
    let term = Term::stdout();
    if yes
        || Confirm::with_theme(&theme())
            .with_prompt("Are you sure you want to clear the log file?")
            .default(false)
            .interact()?
    {
        Nirvana::clear_logs().await?;
        info!("logs cleared");
        term.write_line(&format!("{} the log file", style("Cleared").green()))?;
    } else {
        term.write_line(&format!(
            "{}",
            style("The log file was not cleared").yellow()
        ))?;
    }

    Ok(())
}
