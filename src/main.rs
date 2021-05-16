//! xargs clone
//!
//! # Backlog
//!
//! - TODO: port to Windows.
//!     - Maximum command-line length (hardcoded on Windows).
//!     - Conversions between OsString and [u8].
//! - TODO: implement missing options.
//! - TODO: enhance error handling (do not leave errors escape out of main()...).
//! - TODO: add integration tests.
//! - TODO: Integrate in rust findutils.

use std::{
    iter::Iterator,
    ffi::OsStr,
    io::{self, Read},
};

use anyhow::{Context, bail};
use clap::{App, Arg};

mod parser;
use parser::Parser;

mod children;
use children::ChildMinder;

mod options {
    pub const CMD: &str = "CMD";
    pub const INITIAL_ARGS: &str = "INITIAL_ARGS";
    pub const MAX_BYTES: &str = "MAX_BYTES";
}

fn main() -> anyhow::Result<()> {
    let matches = App::new("xargs")
        .about("Construct argument lists and execute utility.")
        .arg(
            Arg::with_name(options::CMD)
                .help("Utility to run")
                .index(1)
                .default_value("echo"),
        )
        .arg(
            Arg::with_name(options::INITIAL_ARGS)
                .help("Initial arguments passed to CMD")
                .index(2)
                .multiple(true),
        )
        .arg(
            Arg::with_name(options::MAX_BYTES)
                .help("Maximum size of command line passed to utility in bytes")
                .takes_value(true)
                .short("-s"),
        )
        .get_matches();
    let cmd = matches.value_of_os(options::CMD).unwrap();
    let initial_args = matches.values_of_os(options::INITIAL_ARGS).unwrap_or_default();
    let max_cmd_line_len = match matches.value_of(options::MAX_BYTES) {
        Some(s) => s.parse::<usize>().context("Invalid argument to -s")?,
        None => max_os_cmd_line_len(),
    };
    let max_remaining_args_len = max_cmd_line_len as isize - initial_cmd_line_len(cmd, initial_args.clone()) as isize - 1;
    if max_remaining_args_len < 1 {
        bail!("initial command line length ({}) too big for selected maximum size ({})", initial_cmd_line_len(cmd, initial_args.clone()), max_cmd_line_len);
    }

    let mut stdin = io::stdin();
    let mut buf = [0u8];
    let mut minder = ChildMinder::new(1, cmd, initial_args.clone());
    let mut parser = Parser::new(max_remaining_args_len as usize, |args| {
        minder.spawn(args)
    });

    loop {
        match stdin.read(&mut buf[..]) {
            Ok(0) => break,
            Ok(_) => parser.handle_byte(buf[0])?,
            Err(e) => return Err(e).context("Failed to read from stdin"),
        }
    }

    parser.handle_eof()?;

    minder.wait_all()?;

    Ok(())
}

/// Returns length in bytes of `cmd` and `args`.
///
/// TODO: xargs man page states that zero terminators should be counted.
fn initial_cmd_line_len<I>(cmd: &OsStr, args: I) -> usize
where
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    args.into_iter()
        .fold(cmd.len(), |acc, i| acc + i.as_ref().len() + 1)
}

/// Returns maximum length in bytes of command-line (command + all arguments) supported by OS.
///
/// TODO: xargs man page states it uses ARG_MAX - 4096
fn max_os_cmd_line_len() -> usize {
    // SAFETY: No memory safety issue as this function takes and return a scalar.
    let max = unsafe { libc::sysconf(libc::_SC_ARG_MAX) };
    if max == -1 {
        panic!("Cannot get maximum command-line length");
    }
    // TODO: _SC_ARG_MAX is the maximum size of the all argv passed to exec(2) and environment.
    // We do not compute the environment size so reserve some hopefully big enough space for it.
    (max / 2) as usize
}

#[cfg(test)]
mod test_initial_cmd_line_len {
    use super::*;

    #[test]
    fn no_argument() {
        let zero_args: &[&OsStr] = &[];
        assert_eq!(initial_cmd_line_len(OsStr::new("x"), zero_args), 1);
        assert_eq!(initial_cmd_line_len(OsStr::new("yy"), zero_args), 2);
    }

    #[test]
    fn single_argument() {
        assert_eq!(initial_cmd_line_len(OsStr::new("x"), &[OsStr::new("y")]), 3);
        assert_eq!(initial_cmd_line_len(OsStr::new("x"), &[OsStr::new("zz")]), 4);
    }

    #[test]
    fn several_arguments() {
        assert_eq!(initial_cmd_line_len(OsStr::new("x"), &[OsStr::new("y"), OsStr::new("z")]), 5);
    }
}