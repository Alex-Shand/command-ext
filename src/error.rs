use std::process::ExitStatus;

use self::utils::{Exe, ExeAndArgs};

mod utils;

/// Failed to execute target executable
#[errors::error(
    wrapping = std::io::Error,
    context = pub(crate),
    display("Failed to execute `{exe}`"),
    backtrace
)]
pub struct ExecutionError {
    exe: Exe,
}

/// Command returned non-zero exit status
#[errors::error(
    context = pub(crate),
    display("Command `{cmd}` exited unsuccessfully ({status})"),
    backtrace
)]
pub struct StatusError {
    cmd: ExeAndArgs,
    status: ExitStatus,
}

#[allow(missing_docs)]
#[errors::union]
pub type CheckStatusError = (ExecutionError, StatusError);

/// Command returned non-utf8 output
#[errors::error(
    wrapping = std::str::Utf8Error,
    context = pub(crate),
    display("Command `{cmd}` returned non-utf8 output"),
    backtrace
)]
pub struct NonUtf8OutputError {
    cmd: ExeAndArgs,
}

#[allow(missing_docs)]
#[errors::union]
pub type CheckOutputError = (ExecutionError, StatusError, NonUtf8OutputError);
