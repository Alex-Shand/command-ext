use std::process::ExitStatus;

use self::utils::{Exe, ExeAndArgs};

mod utils;

#[errors::error(
    wrapping = std::io::Error,
    context = pub(crate),
    display("Failed to execute `{exe}`"),
    backtrace
)]
pub struct ExecutionError {
    exe: Exe,
}

#[errors::error(
    context = pub(crate),
    display("Command `{cmd}` exited unsuccessfully ({status})"),
    backtrace
)]
pub struct StatusError {
    cmd: ExeAndArgs,
    status: ExitStatus,
}

#[errors::union]
pub type CheckStatusError = (ExecutionError, StatusError);

#[errors::error(
    wrapping = std::str::Utf8Error,
    context = pub(crate),
    display("Command `{cmd}` returned non-utf8 output"),
    backtrace
)]
pub struct NonUtf8OutputError {
    cmd: ExeAndArgs,
}

#[errors::union]
pub type CheckOutputError = (ExecutionError, StatusError, NonUtf8OutputError);
