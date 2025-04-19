//! Extra methods for Command
#![warn(elided_lifetimes_in_paths)]
#![warn(missing_docs)]
#![warn(noop_method_call)]
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

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    net::Ipv4Addr,
    path::Path,
    process::{Command, ExitStatus},
};

use camino::Utf8Path;
use errors::prelude::*;

use self::error::{
    CheckOutputError, CheckStatusError, ExecutionCtx, ExecutionError,
    NonUtf8OutputCtx, StatusCtx,
};

mod error;

/// Extra methods for Command
#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
#[sealed::sealed]
pub trait CommandExt {
    fn check(&mut self) -> Result<ExitStatus, ExecutionError>;
    fn check_status(&mut self) -> Result<(), CheckStatusError>;
    fn check_output(&mut self) -> Result<String, CheckOutputError>;

    #[must_use]
    fn run_as_root(&mut self) -> Self;
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
    fn check(&mut self) -> Result<ExitStatus, ExecutionError> {
        self.status().context(ExecutionCtx { exe: &*self })
    }

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
            .context(NonUtf8OutputCtx { cmd: &*self })?)
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
