use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();

    loop {
        print!("db > ");
        io::stdout().flush().unwrap();

        let mut command = String::new();
        stdin.lock().read_line(&mut command).unwrap();

        let command = command.trim();

        match command {
            ".exit" => break,
            _ => println!("Unrecognized command: {}.", command),
        }
    }
}
