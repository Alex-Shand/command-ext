use std::{ffi::OsString, fmt, process::Command};

#[derive(Debug)]
pub(crate) struct Exe(OsString);

impl fmt::Display for Exe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}

impl From<&Command> for Exe {
    fn from(cmd: &Command) -> Self {
        Self(cmd.get_program().to_owned())
    }
}

#[derive(Debug)]
pub(crate) struct ExeAndArgs {
    exe: OsString,
    args: Vec<OsString>,
}

impl fmt::Display for ExeAndArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.exe.to_string_lossy())?;
        for arg in &self.args {
            write!(f, " {}", arg.to_string_lossy())?;
        }
        Ok(())
    }
}

impl From<&Command> for ExeAndArgs {
    fn from(cmd: &Command) -> Self {
        let exe = cmd.get_program().to_owned();
        let args = cmd.get_args().map(ToOwned::to_owned).collect::<Vec<_>>();
        Self { exe, args }
    }
}
