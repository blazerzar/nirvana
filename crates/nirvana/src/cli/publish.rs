use chrono::{Local, NaiveTime, TimeZone};
use console::Term;
use nirvana_core::api::NirvanaApi;

use crate::cli::{PublishArgs, parse_time};

pub(crate) fn run(args: PublishArgs) -> anyhow::Result<()> {
    let api = NirvanaApi::new()?;

    let today = Local::now().date_naive();
    let from = args
        .start
        .as_deref()
        .map(parse_time)
        .transpose()?
        .unwrap_or_else(|| {
            let dt = today.and_time(NaiveTime::MIN);
            Local.from_local_datetime(&dt).single().unwrap().timestamp()
        });

    let to = args.stop.as_deref().map(parse_time).transpose()?;

    let term = Term::stdout();
    let result = api.publish(from, to)?;

    if result.published_count == 0 && result.failed.is_empty() {
        term.write_line("No unpublished slots found.")?;
        return Ok(());
    }

    if result.published_count > 0 {
        term.write_line(&format!("Published {} slot(s).", result.published_count))?;
    }

    for failure in &result.failed {
        term.write_line(&format!(
            "Failed: {} — {}",
            failure.ticket_key, failure.error
        ))?;
    }

    Ok(())
}
