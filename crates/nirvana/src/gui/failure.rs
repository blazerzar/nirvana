use std::error::Error;
use std::io::{self, ErrorKind};
use std::iter;

use nirvana_core::{DomainError, NirvanaError};

const START_FAILED: &str = "couldn't start nirvana";
const DATABASE_FAILED: &str = "couldn't open the database";
const PROFILE_FAILED: &str = "couldn't load the profile";
const CONFIG_FAILED: &str = "couldn't read the config file";

/// The text of an error toast and the reason shown below it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Failure {
    pub(super) text: &'static str,
    pub(super) detail: Option<String>,
}

impl Failure {
    pub(super) fn new(text: &'static str, detail: Option<&str>) -> Self {
        Self {
            text,
            detail: detail.map(str::to_owned),
        }
    }

    pub(super) fn from_error(text: &'static str, error: &(dyn Error + 'static)) -> Self {
        Self::new(text, domain_reason(error).or_else(|| io_reason(error)))
    }

    pub(super) fn startup(error: &NirvanaError) -> Self {
        let text = match error {
            NirvanaError::Db(_) => DATABASE_FAILED,
            _ => START_FAILED,
        };
        Self::from_error(text, error)
    }

    pub(super) fn profile(error: &NirvanaError) -> Self {
        let text = match error {
            NirvanaError::ConfigUnavailable(_) => CONFIG_FAILED,
            _ => PROFILE_FAILED,
        };
        Self::from_error(text, error)
    }
}

/// Joins the messages of `error` and the errors that caused it, outermost
/// first.
pub(super) fn chain(error: &(dyn Error + 'static)) -> String {
    iter::successors(Some(error), |&e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

/// Returns why a domain error failed, for the errors a user can act on.
fn domain_reason(error: &(dyn Error + 'static)) -> Option<&'static str> {
    match error.downcast_ref::<NirvanaError>()? {
        NirvanaError::Domain(DomainError::RunningSlotJustStarted) => {
            Some("tracking started less than a second ago, try again")
        }
        _ => None,
    }
}

/// Returns why the first I/O error in the chain of `error` failed, for the
/// kinds a user can act on.
fn io_reason(error: &(dyn Error + 'static)) -> Option<&'static str> {
    let io = iter::successors(Some(error), |&e| e.source())
        .find_map(|e| e.downcast_ref::<io::Error>())?;
    match io.kind() {
        ErrorKind::PermissionDenied => Some("no permission to access it"),
        ErrorKind::NotFound => Some("the file doesn't exist"),
        ErrorKind::ReadOnlyFilesystem => Some("the disk is read-only"),
        ErrorKind::StorageFull => Some("the disk is full"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_joins_the_error_and_its_sources() {
        let error = NirvanaError::from(io::Error::other("disk full"));
        assert_eq!(chain(&error), "I/O error: disk full");
    }

    #[test]
    fn io_errors_give_their_kind_as_reason() {
        let error = NirvanaError::from(io::Error::from(ErrorKind::PermissionDenied));
        assert_eq!(
            Failure::startup(&error),
            Failure::new(START_FAILED, Some("no permission to access it"))
        );
    }

    #[test]
    fn running_slot_just_started_gives_a_reason() {
        let error = NirvanaError::from(DomainError::RunningSlotJustStarted);
        assert_eq!(
            Failure::from_error(START_FAILED, &error).detail.as_deref(),
            Some("tracking started less than a second ago, try again")
        );
    }

    #[test]
    fn unknown_errors_have_no_reason() {
        let error = NirvanaError::from(io::Error::other("disk full"));
        assert_eq!(Failure::startup(&error).detail, None);
    }
}
