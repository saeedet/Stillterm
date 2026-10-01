//! The .scr contract: no arguments configure; /s runs; /p embeds in a parent.
use std::num::NonZeroIsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Configure(Option<NonZeroIsize>),
    Fullscreen,
    Preview(NonZeroIsize),
}

pub fn parse(args: &[String]) -> Result<Mode, &'static str> {
    if args.is_empty() {
        return Ok(Mode::Configure(None));
    }
    let first = &args[0];
    let flag = first
        .strip_prefix('/')
        .or_else(|| first.strip_prefix('-'))
        .ok_or("expected /s, /c[:HWND], or /p HWND")?;
    let (flag, inline) = flag
        .split_once(':')
        .map_or((flag, None), |(a, b)| (a, Some(b)));
    let handle = match (inline, args.get(1).map(String::as_str), args.len()) {
        (None, None, 1) => None,
        (Some(value), None, 1) | (None, Some(value), 2) => Some(value),
        _ => return Err("unexpected arguments"),
    };
    let parent = |value: &str| {
        // Windows supplies a decimal HWND. Reject signs, zero, and overflow.
        if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
            return Err("parent window must be a positive decimal handle");
        }
        value
            .parse::<isize>()
            .ok()
            .and_then(NonZeroIsize::new)
            .ok_or("parent window handle is zero or out of range")
    };
    match flag.to_ascii_lowercase().as_str() {
        "s" if handle.is_none() => Ok(Mode::Fullscreen),
        "c" => Ok(Mode::Configure(handle.map(parent).transpose()?)),
        "p" => Ok(Mode::Preview(parent(
            handle.ok_or("preview requires a parent window")?,
        )?)),
        _ => Err("expected /s, /c[:HWND], or /p HWND"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mode(args: &[&str]) -> Result<Mode, &'static str> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
    #[test]
    fn accepts_windows_modes_and_separators() {
        assert_eq!(mode(&[]), Ok(Mode::Configure(None)));
        assert_eq!(mode(&["/S"]), Ok(Mode::Fullscreen));
        assert_eq!(mode(&["-c"]), Ok(Mode::Configure(None)));
        for args in [["/p", "1234"], ["-P", "1234"]] {
            assert_eq!(
                mode(&args),
                Ok(Mode::Preview(NonZeroIsize::new(1234).unwrap()))
            );
        }
        assert_eq!(mode(&["/P:1234"]), mode(&["/p", "1234"]));
        assert_eq!(mode(&["/c:1234"]), mode(&["/C", "1234"]));
    }
    #[test]
    fn malformed_invocation_never_starts_fullscreen() {
        for args in [
            vec!["/p"],
            vec!["/p", "0"],
            vec!["/p", "-1"],
            vec!["/p", "+1"],
            vec!["/c:"],
            vec!["/p:1", "2"],
            vec!["/s", "123"],
            vec!["/x"],
            vec!["s"],
            vec!["/p", "18446744073709551616"],
        ] {
            assert!(mode(&args).is_err(), "{args:?}");
        }
    }
}
