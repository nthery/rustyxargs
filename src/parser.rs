//! Stdin parser.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

/// Breaks down input bytes into space-separated arguments and accumulates them
/// until maximum size reached.
///
/// - TODO: Quoting.
/// - TODO: Zero-separated words.
pub(crate) struct Parser<F>
where
    F: FnMut(&[OsString]) -> anyhow::Result<()>,
{
    /// All arguments accumulated so far.
    args: Vec<OsString>,

    /// Current length in bytes of arguments in `args` including separators.
    cur_len: usize,

    /// Maximum length in bytes of all arguments.
    max_len: usize,

    /// Argument being parsed.
    arg: Vec<u8>,

    /// Closure called when concatenating `arg` to `args` would exceed `max_len`.
    action: F,
}

impl<F: FnMut(&[OsString]) -> anyhow::Result<()>> Parser<F> {
    /// Creates a new parser that will accumulate arguments up to `max_len`
    /// bytes and repeatedly call `action` with accumulated arguments.
    pub fn new(max_len: usize, action: F) -> Self {
        Self {
            max_len,
            args: Vec::new(),
            cur_len: 0,
            arg: Vec::new(),
            action,
        }
    }

    /// Parses incoming byte.
    pub fn handle_byte(&mut self, ch: u8) -> anyhow::Result<()> {
        if (ch as char).is_ascii_whitespace() {
            self.handle_space()?;
        } else {
            self.arg.push(ch);
        }
        Ok(())
    }

    /// Flushes accumulated arguments on EOF.
    pub fn handle_eof(&mut self) -> anyhow::Result<()> {
        self.handle_space()?;
        if !self.args.is_empty() {
            (self.action)(&self.args)?;
        }
        Ok(())
    }

    fn handle_space(&mut self) -> anyhow::Result<()> {
        // TODO handle single arg too long
        if self.is_break_down_needed() {
            (self.action)(&self.args)?;
            self.args.clear();
            self.cur_len = 0;
        }
        if !self.arg.is_empty() {
            self.append_arg();
        }
        Ok(())
    }

    fn is_break_down_needed(&self) -> bool {
        let separator_len = if !self.args.is_empty() { 1 } else { 0 };
        self.cur_len + separator_len + self.arg.len() > self.max_len
    }

    fn append_arg(&mut self) {
        if !self.args.is_empty() {
            self.cur_len += 1;
        }
        self.args.push(OsStr::from_bytes(&self.arg).to_owned());
        self.cur_len += self.arg.len();
        self.arg.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds parser for `max_len`, passes it `input` and returns the parsed
    /// arguments as an array of lines, each line containing the arguments for
    /// one invocation.
    fn run(max_len: usize, input: &[u8]) -> anyhow::Result<Vec<Vec<String>>> {
        let mut lines = Vec::<Vec<String>>::new();
        let mut p = Parser::new(max_len, |args| {
            lines.push(args.iter().map(|oss| oss.to_str().unwrap().to_owned()).collect());
            Ok(())
        });
        for b in input {
            p.handle_byte(*b)?;
        }
        p.handle_eof()?;
        Ok(lines)
    }

    #[test]
    fn empty() -> anyhow::Result<()> {
        assert_eq!(run(42, b"")?, Vec::<Vec<String>>::new());
        Ok(())
    }

    #[test]
    fn minimal_input() -> anyhow::Result<()> {
        assert_eq!(run(42, b"x")?, [["x"]]);
        Ok(())
    }

    #[test]
    fn all_args_fit_in_single_line() -> anyhow::Result<()> {
        assert_eq!(run(3, b"x y")?, [["x", "y"]]);
        assert_eq!(run(3, b"x y ")?, [["x", "y"]]);
        Ok(())
    }

    #[test]
    fn break_down_needed() -> anyhow::Result<()> {
        assert_eq!(run(3, b"x yz")?, [["x"], ["yz"]]);
        Ok(())
    }
}
