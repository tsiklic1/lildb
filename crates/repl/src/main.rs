use std::{
    io::{self, BufRead, Write},
    os::macos::raw::stat,
};

use anyhow::Error;

use crate::StatementType::Insert;

fn main() {
    let stdin = io::stdin();

    loop {
        print!("lildb > ");
        io::stdout().flush().unwrap();

        let mut command = String::new();
        stdin.lock().read_line(&mut command).unwrap();

        let command = command.trim();

        match command.as_bytes()[0] {
            b'.' => match command {
                //meta command
                ".exit" => break,
                _ => println!("Unrecognized meta command: {}.", command),
            },
            //statement
            _ => {
                let prepare_result = prepare_statement(command);
                match prepare_result {
                    Ok(statement) => {
                        execute_statement(statement);
                    }
                    Err(()) => println!("Unrecognized statement keyword: {}", command),
                }
            }
        }
    }
}

fn prepare_statement(input: &str) -> Result<Statement, ()> {
    let mut statement = Statement {
        statement_type: StatementType::Select,
    };
    if input.starts_with("insert") {
        statement.statement_type = StatementType::Insert;
        return Ok(statement);
    }
    if input.starts_with("select") {
        statement.statement_type = StatementType::Select;
        return Ok(statement);
    }
    println!("abc");
    Err(())
}

fn execute_statement(statement: Statement) {
    match statement.statement_type {
        StatementType::Insert => println!("Here we will do the insert"),
        StatementType::Select => println!("Here we will do the select"),
    }
}

enum StatementType {
    Insert,
    Select,
}

struct Statement {
    statement_type: StatementType,
}
