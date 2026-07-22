mod constants;
mod db;
mod types;

use std::io::{self, BufRead, Write};

use db::{db_close, db_open, execute_statement, prepare_statement, print_constants, print_tree};

fn main() {
    let stdin = io::stdin();

    println!("Enter db filename");
    let mut filename = String::new();
    stdin.lock().read_line(&mut filename).unwrap();
    let mut table = db_open(&filename.trim());

    loop {
        print!("lildb > ");
        io::stdout().flush().unwrap();

        let mut command = String::new();
        stdin.lock().read_line(&mut command).unwrap();

        let command = command.trim();

        match command.as_bytes()[0] {
            b'.' => match command {
                //meta command
                ".exit" => {
                    db_close(&mut table);
                    break;
                }
                ".constants" => {
                    println!("Constants: ");
                    print_constants();
                }
                ".btree" => {
                    println!("Tree:");
                    let root_page_num = table.root_page_num;
                    print_tree(&mut table.pager, root_page_num, 0);
                }
                _ => println!("Unrecognized meta command: {}.", command),
            },
            //statement
            _ => {
                let prepare_result = prepare_statement(command);
                match prepare_result {
                    Ok(statement) => {
                        execute_statement(statement, &mut table);
                    }
                    Err(()) => println!("Unrecognized statement keyword: {}", command),
                }
            }
        }
    }
}
