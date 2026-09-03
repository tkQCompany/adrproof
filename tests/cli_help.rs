use std::process::Command;

const COMMANDS: &[&str] = &[
    "check",
    "facts",
    "explain",
    "impact",
    "status",
    "diagnose",
    "scenario",
    "native-test",
    "provider",
    "bundle",
    "model",
    "correspondence",
];

fn adrproof() -> Command {
    Command::new(env!("CARGO_BIN_EXE_adrproof"))
}

#[test]
fn root_help_is_successful_and_lists_every_command() {
    let output = adrproof().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("Usage: adrproof COMMAND"));
    for command in COMMANDS {
        assert!(stdout.lines().any(|line| line.trim() == *command));
    }
}

#[test]
fn every_command_has_side_effect_free_help() {
    for command in COMMANDS {
        let output = adrproof().args([command, "--help"]).output().unwrap();
        assert!(output.status.success(), "{command}: {output:?}");
        assert!(output.stderr.is_empty(), "{command}: {output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.starts_with(&format!("Usage: adrproof {command}")),
            "unexpected help for {command}: {stdout}"
        );
    }
}
