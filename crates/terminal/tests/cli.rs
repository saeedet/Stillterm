use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_stillterm"))
}

#[test]
fn help_and_list_work_without_a_terminal() {
    let help = cli().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--fps"));
    let list = cli().arg("list").output().unwrap();
    assert!(list.status.success());
    assert!(String::from_utf8_lossy(&list.stdout).starts_with("rain"));
    assert!(!list.stdout.contains(&0x1b));
}

#[test]
fn redirected_animation_is_rejected_without_escape_sequences() {
    let output = cli().output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
}

#[test]
fn invalid_configuration_fails_before_terminal_setup() {
    for flags in [
        ["--fps", "0"],
        ["--effect", "unknown"],
        ["--theme", "unknown"],
        ["--intensity", "NaN"],
        ["--characters", "\x1b"],
        ["--words", "a\tb"],
        ["--word-frequency", "sometimes"],
    ] {
        let output = cli().args(flags).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.contains(&0x1b));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    }
}

#[test]
fn configuration_file_errors_identify_the_file_before_terminal_setup() {
    struct TemporaryDirectory(std::path::PathBuf);
    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = TemporaryDirectory(
        std::env::temp_dir().join(format!("stillterm-config-{}-{nonce}", std::process::id())),
    );
    std::fs::create_dir(&directory.0).unwrap();
    let path = directory.0.join("settings.toml");
    let check = |expected: &str| {
        let output = cli().arg("--config").arg(&path).output().unwrap();
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.contains(&0x1b));
        assert!(error.contains(path.to_str().unwrap()), "{error}");
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("interactive terminal"), "{error}");
    };
    check("cannot read");
    for (bytes, expected) in [
        (vec![0xff], "must be UTF-8"),
        (b"theme = [".to_vec(), "invalid configuration"),
        (vec![b' '; 65_537], "at most 64 KiB"),
        // The bounded read ends inside a valid multibyte character. Size takes
        // precedence over encoding when the file exceeds the byte limit.
        (
            [vec![b' '; 65_536], "雨".as_bytes().to_vec()].concat(),
            "at most 64 KiB",
        ),
    ] {
        std::fs::write(&path, bytes).unwrap();
        check(expected);
    }
}
