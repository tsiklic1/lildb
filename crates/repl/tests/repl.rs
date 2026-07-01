use std::io::Write;
use std::process::{Command, Stdio};

fn run_script(commands: &[&str]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_repl"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start repl binary");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        for command in commands {
            writeln!(stdin, "{command}").expect("failed to write command");
        }
    }

    let output = child.wait_with_output().expect("failed to read output");
    assert!(
        output.status.success(),
        "repl exited with status {:?}",
        output.status.code()
    );

    String::from_utf8(output.stdout).expect("repl output was not valid utf-8")
}

#[test]
fn inserts_and_retrieves_a_row() {
    let output = run_script(&["insert 1 user1 person1@example.com", "select", ".exit"]);

    assert!(output.contains("lildb >"));
    assert!(output.contains("1"));
    assert!(output.contains("user1"));
    assert!(output.contains("person1@example.com"));
}

#[test]
fn allows_inserting_strings_that_are_the_maximum_length() {
    let long_username = "a".repeat(32);
    let long_email = "a".repeat(255);
    let insert_command = format!("insert 1 {long_username} {long_email}");

    let output = run_script(&[insert_command.as_str(), "select", ".exit"]);

    assert!(output.contains(&format!("1 {long_username} {long_email}")));
}

#[test]
fn prints_error_message_if_id_is_negative() {
    let output = run_script(&["insert -1 cstack foo@bar.com", "select", ".exit"]);

    assert!(output.contains("lildb > ID must be positive."));
}

#[test]
fn prints_error_message_when_table_is_full() {
    let mut commands = Vec::new();
    for i in 1..=1401 {
        commands.push(format!("insert {i} user{i} person{i}@example.com"));
    }
    commands.push(".exit".to_string());

    let command_refs: Vec<&str> = commands.iter().map(String::as_str).collect();
    let output = run_script(&command_refs);

    assert!(output.contains("lildb > Error: Table full."));
}
