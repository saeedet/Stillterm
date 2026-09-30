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
    ] {
        let output = cli().args(flags).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.contains(&0x1b));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    }
}
