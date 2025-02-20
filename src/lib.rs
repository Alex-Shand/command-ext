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

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    net::Ipv4Addr,
    path::Path,
    process::{Command, ExitStatus},
};

use anyhow::{Context as _, Error, Result, anyhow};

/// Extra methods for Command
#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub trait CommandExt {
    fn check(&mut self) -> Result<ExitStatus>;
    fn check_status(&mut self) -> Result<()>;
    fn check_output(&mut self) -> Result<String>;

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
    fn redirect(&mut self, path: impl AsRef<Path>) -> Self;
}

impl CommandExt for Command {
    fn check(&mut self) -> Result<ExitStatus> {
        self.status().with_context(|| failed_to_execute(self))
    }

    fn check_status(&mut self) -> Result<()> {
        let status = self.status().with_context(|| failed_to_execute(self))?;
        if !status.success() {
            return Err(unsuccessful_exit(self, status));
        }
        Ok(())
    }

    fn check_output(&mut self) -> Result<String> {
        let output = self.output().with_context(|| failed_to_execute(self))?;
        let status = output.status;
        if !status.success() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(unsuccessful_exit(self, status));
        }
        String::from_utf8(output.stdout).with_context(|| non_utf8_output(self))
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
        let cmd = convert_to_commandline(self);
        let mut new = Command::new("ssh");
        let _ = new
            .arg("-i")
            .arg(key_file.as_ref())
            .arg(format!("{user}@{ip}"))
            .arg(cmd);
        new
    }

    #[allow(clippy::similar_names)]
    fn redirect(&mut self, path: impl AsRef<Path>) -> Self {
        let cmd = convert_to_commandline(self);
        let cmd = format!(
            "{cmd} >{}",
            shellwords::escape(
                path.as_ref().as_os_str().to_str().expect("Non-utf8 path")
            )
        );
        let mut new = Command::new("bash");
        let _ = new.arg("-c").arg(cmd);
        new
    }
}

fn failed_to_execute(cmd: &mut Command) -> String {
    let exe = cmd.get_program().to_string_lossy();
    format!("Failed to execute `{exe}`")
}

fn unsuccessful_exit(cmd: &mut Command, status: ExitStatus) -> Error {
    let exe = cmd.get_program().to_string_lossy();
    let args = cmd
        .get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    anyhow!("Command `{exe} {args}` exited unsuccessfully ({status})")
}

fn non_utf8_output(cmd: &mut Command) -> String {
    let exe = cmd.get_program().to_string_lossy();
    let args = cmd
        .get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    format!("Command `{exe} {args}` returned non-utf8 output")
}

#[allow(clippy::similar_names)]
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

#[allow(clippy::similar_names)]
fn convert_to_commandline(cmd: &Command) -> String {
    let (exe, args, cwd, env_set, env_del) = decompose_command(cmd);
    let exe = shellwords::escape(
        exe.to_str()
            .expect("Can't send non-utf8 commands across ssh"),
    );
    let args = shellwords::join(
        &args
            .map(|a| {
                a.to_str().expect("Can't send non-utf8 commands across ssh")
            })
            .collect::<Vec<_>>(),
    );
    let env_set = env_set
        .into_iter()
        .map(|(k, v)| {
            format!(
                "export {}={};",
                k.to_str()
                    .expect("Can't send non-utf8 environment across ssh"),
                shellwords::escape(
                    v.to_str()
                        .expect("Can't send non-utf8 environment across ssh")
                )
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let env_del = env_del
        .into_iter()
        .map(|e| {
            format!(
                "unset {};",
                e.to_str()
                    .expect("Can't send non-utf8 environment across ssh")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let cwd = if let Some(cwd) = cwd {
        format!(
            "cd {};",
            shellwords::escape(
                cwd.as_os_str()
                    .to_str()
                    .expect("Can't send non-utf8 environment across ssh")
            )
        )
    } else {
        String::new()
    };

    format!("{env_set} {env_del} {cwd} {exe} {args}")
}
