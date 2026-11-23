use std::io::ErrorKind;
use std::sync::Arc;

use anyhow::anyhow;
use console::{Term, style};
use nirvana_core::Nirvana;
use tracing::error;

use crate::web;

pub(crate) async fn run() -> anyhow::Result<()> {
    let nirvana = Nirvana::new().await?;
    let port = nirvana.port().await?;
    let listener = web::bind(port).await.map_err(|e| match e.kind() {
        ErrorKind::AddrInUse => {
            anyhow!("{e}, set another one with `nirvana config set core.port <port>`")
        }
        _ => anyhow::Error::new(e).context(format!("could not listen on port {port}")),
    })?;

    Term::stdout().write_line(&format!(
        "{} nirvana web at {}",
        style("Serving").green(),
        style(web::url(port)).bold(),
    ))?;

    web::run(listener, Arc::new(nirvana), async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("could not listen for ctrl-c: {e}");
        }
    })
    .await?;

    Ok(())
}
