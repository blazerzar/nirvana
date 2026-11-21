use console::{Style, Term};
use nirvana_core::Nirvana;

use crate::about;

pub(crate) async fn run() -> anyhow::Result<()> {
    let api = Nirvana::new().await?;
    let info = api.info().await?;
    let term = Term::stdout();
    let bold = Style::new().bold();

    term.write_line(&format!("Nirvana {}", bold.apply_to(about::version(&info))))?;
    term.write_line("")?;
    for (label, value) in about::rows(&info) {
        term.write_line(&format!("{:16}{}", bold.apply_to(label), value))?;
    }

    Ok(())
}
