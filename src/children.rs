//! subprocesses management

use anyhow::Context;
use std::{
    ffi::{OsStr, OsString},
    mem,
    process::{Child, Command, Stdio},
};

/// Invokes and manages set of child processes.
pub(crate) struct ChildMinder {
    /// Maximum number of children operating in parallel.
    max_children: usize,

    /// Utility to invoke.
    cmd: OsString,

    /// Initial arguments passed to utility.
    initial_args: Vec<OsString>,

    /// All running children.
    children: Vec<Child>,
}

impl ChildMinder {
    /// Creates a new `ChildMinder` that will invoke `cmd` with `initial_args`
    /// and more arguments that will be specified in `spawn()`.
    pub fn new<I>(max_children: usize, cmd: &OsStr, initial_args: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        debug_assert!(max_children > 0);
        Self {
            max_children,
            cmd: cmd.to_owned(),
            initial_args: initial_args
                .into_iter()
                .map(|i| i.as_ref().to_owned())
                .collect(),
            children: Vec::new(),
        }
    }

    /// Runs a child process with the arguments specified in `new()` and `remaining_args`.
    ///
    /// May block if the maximum number of processes has been reached.
    pub fn spawn(&mut self, remaining_args: &[OsString]) -> anyhow::Result<()> {
        if self.children.len() >= self.max_children {
            // TODO: Naive as the oldest child is not necessarily the one that will end up first.
            let mut child = self.children.swap_remove(0);
            child.wait()?;
            // TODO: log error/bail out if the child failed.
        }
        let child = Command::new(&self.cmd)
            .args(&self.initial_args)
            .args(remaining_args)
            .stdin(Stdio::null())
            .spawn()
            .context("Can not start child process")?;
        self.children.push(child);
        Ok(())
    }

    pub fn wait_all(&mut self) -> anyhow::Result<()> {
        // Take ownership of children to avoid iterating them again in drop().
        for mut c in mem::take(&mut self.children) {
            c.wait().context("Waiting for child process failed")?;
        }
        Ok(())
    }
}

impl Drop for ChildMinder {
    fn drop(&mut self) {
        self.wait_all().unwrap();
    }
}
