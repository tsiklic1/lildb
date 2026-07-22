use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_script(commands: &[&str]) -> String {
    let db_path = std::env::temp_dir().join(format!(
        "lildb-test-{}-{}.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time was before unix epoch")
            .as_nanos()
    ));

    let mut child = Command::new(env!("CARGO_BIN_EXE_repl"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start repl binary");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        writeln!(stdin, "{}", db_path.display()).expect("failed to write db filename");
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

    let _ = fs::remove_file(db_path);

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

#[test]
fn allows_printing_out_the_structure_of_a_4_leaf_node_btree() {
    let output = run_script(&[
        "insert 18 user18 person18@example.com",
        "insert 7 user7 person7@example.com",
        "insert 10 user10 person10@example.com",
        "insert 29 user29 person29@example.com",
        "insert 23 user23 person23@example.com",
        "insert 4 user4 person4@example.com",
        "insert 14 user14 person14@example.com",
        "insert 30 user30 person30@example.com",
        "insert 15 user15 person15@example.com",
        "insert 26 user26 person26@example.com",
        "insert 22 user22 person22@example.com",
        "insert 19 user19 person19@example.com",
        "insert 2 user2 person2@example.com",
        "insert 1 user1 person1@example.com",
        "insert 21 user21 person21@example.com",
        "insert 11 user11 person11@example.com",
        "insert 6 user6 person6@example.com",
        "insert 20 user20 person20@example.com",
        "insert 5 user5 person5@example.com",
        "insert 8 user8 person8@example.com",
        "insert 9 user9 person9@example.com",
        "insert 3 user3 person3@example.com",
        "insert 12 user12 person12@example.com",
        "insert 27 user27 person27@example.com",
        "insert 17 user17 person17@example.com",
        "insert 16 user16 person16@example.com",
        "insert 13 user13 person13@example.com",
        "insert 24 user24 person24@example.com",
        "insert 25 user25 person25@example.com",
        "insert 28 user28 person28@example.com",
        ".btree",
        ".exit",
    ]);

    let expected = "\
Tree:
- internal (size 3)
    - leaf (size 7)
        - 1
        - 2
        - 3
        - 4
        - 5
        - 6
        - 7
    - key 7
    - leaf (size 8)
        - 8
        - 9
        - 10
        - 11
        - 12
        - 13
        - 14
        - 15
    - key 15
    - leaf (size 7)
        - 16
        - 17
        - 18
        - 19
        - 20
        - 21
        - 22
    - key 22
    - leaf (size 8)
        - 23
        - 24
        - 25
        - 26
        - 27
        - 28
        - 29
        - 30";

    assert!(output.contains(expected), "actual output:\n{output}");
}

#[test]
fn allows_printing_out_the_structure_of_a_7_leaf_node_btree() {
    let output = run_script(&[
        "insert 58 user58 person58@example.com",
        "insert 56 user56 person56@example.com",
        "insert 8 user8 person8@example.com",
        "insert 54 user54 person54@example.com",
        "insert 77 user77 person77@example.com",
        "insert 7 user7 person7@example.com",
        "insert 25 user25 person25@example.com",
        "insert 71 user71 person71@example.com",
        "insert 13 user13 person13@example.com",
        "insert 22 user22 person22@example.com",
        "insert 53 user53 person53@example.com",
        "insert 51 user51 person51@example.com",
        "insert 59 user59 person59@example.com",
        "insert 32 user32 person32@example.com",
        "insert 36 user36 person36@example.com",
        "insert 79 user79 person79@example.com",
        "insert 10 user10 person10@example.com",
        "insert 33 user33 person33@example.com",
        "insert 20 user20 person20@example.com",
        "insert 4 user4 person4@example.com",
        "insert 35 user35 person35@example.com",
        "insert 76 user76 person76@example.com",
        "insert 49 user49 person49@example.com",
        "insert 24 user24 person24@example.com",
        "insert 70 user70 person70@example.com",
        "insert 48 user48 person48@example.com",
        "insert 39 user39 person39@example.com",
        "insert 15 user15 person15@example.com",
        "insert 47 user47 person47@example.com",
        "insert 30 user30 person30@example.com",
        "insert 86 user86 person86@example.com",
        "insert 31 user31 person31@example.com",
        "insert 68 user68 person68@example.com",
        "insert 37 user37 person37@example.com",
        "insert 66 user66 person66@example.com",
        "insert 63 user63 person63@example.com",
        "insert 40 user40 person40@example.com",
        "insert 78 user78 person78@example.com",
        "insert 19 user19 person19@example.com",
        "insert 46 user46 person46@example.com",
        "insert 14 user14 person14@example.com",
        "insert 81 user81 person81@example.com",
        "insert 72 user72 person72@example.com",
        "insert 6 user6 person6@example.com",
        "insert 50 user50 person50@example.com",
        "insert 85 user85 person85@example.com",
        "insert 67 user67 person67@example.com",
        "insert 2 user2 person2@example.com",
        "insert 55 user55 person55@example.com",
        "insert 69 user69 person69@example.com",
        "insert 5 user5 person5@example.com",
        "insert 65 user65 person65@example.com",
        "insert 52 user52 person52@example.com",
        "insert 1 user1 person1@example.com",
        "insert 29 user29 person29@example.com",
        "insert 9 user9 person9@example.com",
        "insert 43 user43 person43@example.com",
        "insert 75 user75 person75@example.com",
        "insert 21 user21 person21@example.com",
        "insert 82 user82 person82@example.com",
        "insert 12 user12 person12@example.com",
        "insert 18 user18 person18@example.com",
        "insert 60 user60 person60@example.com",
        "insert 44 user44 person44@example.com",
        ".btree",
        ".exit",
    ]);

    let expected = "\
Tree:
- internal (size 1)
    - internal (size 2)
        - leaf (size 7)
            - 1
            - 2
            - 4
            - 5
            - 6
            - 7
            - 8
        - key 8
        - leaf (size 11)
            - 9
            - 10
            - 12
            - 13
            - 14
            - 15
            - 18
            - 19
            - 20
            - 21
            - 22
        - key 22
        - leaf (size 8)
            - 24
            - 25
            - 29
            - 30
            - 31
            - 32
            - 33
            - 35
    - key 35
    - internal (size 3)
        - leaf (size 12)
            - 36
            - 37
            - 39
            - 40
            - 43
            - 44
            - 46
            - 47
            - 48
            - 49
            - 50
            - 51
        - key 51
        - leaf (size 11)
            - 52
            - 53
            - 54
            - 55
            - 56
            - 58
            - 59
            - 60
            - 63
            - 65
            - 66
        - key 66
        - leaf (size 7)
            - 67
            - 68
            - 69
            - 70
            - 71
            - 72
            - 75
        - key 75
        - leaf (size 8)
            - 76
            - 77
            - 78
            - 79
            - 81
            - 82
            - 85
            - 86";

    assert!(output.contains(expected), "actual output:\n{output}");
}
