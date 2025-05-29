//! Extra methods for Command
#![warn(elided_lifetimes_in_paths)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused_crate_dependencies)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]
#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unused_results)]
#![deny(missing_debug_implementations)]
#![deny(missing_copy_implementations)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::let_underscore_untyped)]
#![allow(clippy::similar_names)]
#![allow(clippy::result_large_err)]
#![allow(clippy::missing_errors_doc)]

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    net::Ipv4Addr,
    path::Path,
    process::{Command, ExitStatus, Output as StdOutput},
};

use camino::Utf8Path;
use error::NonUtf8OutputError;
use errors::prelude::*;

use self::error::{
    CheckOutputError, CheckStatusError, ExecutionCtx, ExecutionError,
    NonUtf8OutputCtx, StatusCtx,
};

mod error;

/// Output type returned from [CommandExt::check_full_output]
#[derive(Debug)]
pub struct Output<'a> {
    /// Command exit status
    pub status: ExitStatus,
    output: StdOutput,
    cmd: &'a Command,
}

impl<'a> Output<'a> {
    fn new(cmd: &'a Command, output: StdOutput) -> Self {
        Self {
            status: output.status,
            output,
            cmd,
        }
    }
}

impl Output<'_> {
    /// Command stdout, must be utf8
    pub fn stdout(&self) -> Result<&str, NonUtf8OutputError> {
        std::str::from_utf8(&self.output.stdout)
            .context(NonUtf8OutputCtx { cmd: self.cmd })
    }

    /// Command stdout, lossily converted to string
    #[must_use]
    pub fn stdout_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.output.stdout)
    }

    /// Command stdout, raw bytes
    #[must_use]
    pub fn stdout_raw(&self) -> &[u8] {
        &self.output.stdout
    }

    /// Command stderr, must be utf8
    pub fn stderr(&self) -> Result<&str, NonUtf8OutputError> {
        std::str::from_utf8(&self.output.stderr)
            .context(NonUtf8OutputCtx { cmd: self.cmd })
    }

    /// Command stderr, lossily converted to string
    #[must_use]
    pub fn stderr_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.output.stderr)
    }

    /// Command stderr, raw bytes
    #[must_use]
    pub fn stderr_raw(&self) -> &[u8] {
        &self.output.stderr
    }
}

/// Extra methods for Command
#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
#[sealed::sealed]
pub trait CommandExt {
    fn check(&mut self) -> Result<ExitStatus, ExecutionError>;
    fn check_status(&mut self) -> Result<(), CheckStatusError>;
    fn check_output(&mut self) -> Result<String, CheckOutputError>;
    fn check_full_output(&mut self) -> Result<Output<'_>, ExecutionError>;

    #[must_use]
    fn run_as_root(&mut self) -> Self;
    #[must_use]
    fn run_as(&mut self, user: &str) -> Self;
    #[must_use]
    fn run_on_remote(
        &mut self,
        user: &str,
        ip: Ipv4Addr,
        key_file: impl AsRef<Path>,
    ) -> Self;
    #[must_use]
    fn redirect(&mut self, path: impl AsRef<Utf8Path>) -> Self;
}

#[sealed::sealed]
impl CommandExt for Command {
    #[track_caller]
    fn check(&mut self) -> Result<ExitStatus, ExecutionError> {
        self.status().context(ExecutionCtx { exe: &*self })
    }

    #[track_caller]
    fn check_status(&mut self) -> Result<(), CheckStatusError> {
        let status = self.check()?;
        if !status.success() {
            return StatusCtx {
                cmd: &*self,
                status,
            }
            .fail()?;
        }
        Ok(())
    }

    #[track_caller]
    fn check_output(&mut self) -> Result<String, CheckOutputError> {
        let output = self.output().context(ExecutionCtx { exe: &*self })?;
        let status = output.status;
        if !status.success() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            return StatusCtx {
                cmd: &*self,
                status,
            }
            .fail()?;
        }
        Ok(String::from_utf8(output.stdout)
            .map_err(|e| e.utf8_error())
            .context(NonUtf8OutputCtx { cmd: &*self })?)
    }

    #[track_caller]
    fn check_full_output(&mut self) -> Result<Output<'_>, ExecutionError> {
        let output = self.output().context(ExecutionCtx { exe: &*self })?;
        Ok(Output::new(self, output))
    }

    fn run_as_root(&mut self) -> Self {
        let (exe, args, cwd, env_set, env_del) = decompose_command(self);
        let mut new = Command::new("sudo");
        let _ = new.arg(exe).args(args);
        if let Some(cwd) = cwd {
            let _ = new.current_dir(cwd);
        }
        let _ = new.envs(env_set);
        for e in env_del {
            let _ = new.env_remove(e);
        }
        new
    }

    fn run_as(&mut self, user: &str) -> Self {
        let (exe, args, cwd, env_set, env_del) = decompose_command(self);
        let mut new = Command::new("sudo");
        let _ = new.args(["-u", user]).arg(exe).args(args);
        if let Some(cwd) = cwd {
            let _ = new.current_dir(cwd);
        }
        let _ = new.envs(env_set);
        for e in env_del {
            let _ = new.env_remove(e);
        }
        new
    }

    fn run_on_remote(
        &mut self,
        user: &str,
        ip: Ipv4Addr,
        key_file: impl AsRef<Path>,
    ) -> Self {
        let cmd = convert_to_commandline(self, "run_on_remote");
        let mut new = Command::new("ssh");
        let _ = new
            .arg("-i")
            .arg(key_file.as_ref())
            .arg(format!("{user}@{ip}"))
            .arg(cmd);
        new
    }

    fn redirect(&mut self, path: impl AsRef<Utf8Path>) -> Self {
        let cmd = convert_to_commandline(self, "redirect");
        let cmd =
            format!("{cmd} >{}", shellwords::escape(path.as_ref().as_str()));
        let mut new = Command::new("bash");
        let _ = new.arg("-c").arg(cmd);
        new
    }
}

fn decompose_command(
    cmd: &Command,
) -> (
    &OsStr,
    impl Iterator<Item = &OsStr>,
    Option<&Path>,
    HashMap<&OsStr, &OsStr>,
    HashSet<&OsStr>,
) {
    let exe = cmd.get_program();
    let args = cmd.get_args();
    let cwd = cmd.get_current_dir();
    let env = cmd.get_envs();
    let mut env_set = HashMap::new();
    let mut env_del = HashSet::new();
    for (k, v) in env {
        if let Some(v) = v {
            let _ = env_set.insert(k, v);
        } else {
            let _ = env_del.insert(k);
        }
    }
    (exe, args, cwd, env_set, env_del)
}

fn convert_to_commandline(cmd: &Command, purpose: &'static str) -> String {
    let err =
        || panic!("{purpose} requires all parts of the command be valid utf8");
    let (exe, args, cwd, env_set, env_del) = decompose_command(cmd);
    let exe = shellwords::escape(exe.to_str().unwrap_or_else(err));
    let args = shellwords::join(
        &args
            .map(|a| a.to_str().unwrap_or_else(err))
            .collect::<Vec<_>>(),
    );
    let env_set = env_set
        .into_iter()
        .map(|(k, v)| {
            format!(
                "export {}={};",
                k.to_str().unwrap_or_else(err),
                shellwords::escape(v.to_str().unwrap_or_else(err))
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let env_del = env_del
        .into_iter()
        .map(|e| format!("unset {};", e.to_str().unwrap_or_else(err)))
        .collect::<Vec<_>>()
        .join(" ");
    let cwd = if let Some(cwd) = cwd {
        format!(
            "cd {};",
            shellwords::escape(cwd.as_os_str().to_str().unwrap_or_else(err))
        )
    } else {
        String::new()
    };

    format!("{env_set} {env_del} {cwd} {exe} {args}")
}
