use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();

    let mut table = new_table();

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
                        execute_statement(statement, &mut table);
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
        row_to_insert: Row {
            id: 0,
            username: [0; USERNAME_SIZE],
            email: [0; EMAIL_SIZE],
        },
    };
    if input.starts_with("insert") {
        let mut parts = input.trim().split_whitespace();
        let _command = parts.next();
        let id = parts.next().ok_or(())?;
        let username = parts.next().ok_or(())?;
        let email = parts.next().ok_or(())?;

        let statement = Statement {
            statement_type: StatementType::Insert,
            row_to_insert: Row {
                id: id.parse::<u32>().map_err(|_| ())?,
                username: fixed_bytes(username)?,
                email: fixed_bytes(email)?,
            },
        };
        return Ok(statement);
    }
    if input.starts_with("select") {
        statement.statement_type = StatementType::Select;
        return Ok(statement);
    }
    Err(())
}

enum StatementType {
    Insert,
    Select,
}

struct Statement {
    statement_type: StatementType,
    row_to_insert: Row,
}

struct Row {
    id: u32,
    username: [u8; USERNAME_SIZE],
    email: [u8; EMAIL_SIZE],
}

struct Table {
    num_rows: u32,
    pages: [Option<Box<[u8; PAGE_SIZE]>>; TABLE_MAX_PAGES],
}

const ID_SIZE: usize = 4;
const USERNAME_SIZE: usize = 32;
const EMAIL_SIZE: usize = 255;
const ID_OFFSET: usize = 0;
const USERNAME_OFFSET: usize = ID_OFFSET + ID_SIZE;
const EMAIL_OFFSET: usize = USERNAME_OFFSET + USERNAME_SIZE;
const ROW_SIZE: usize = ID_SIZE + USERNAME_SIZE + EMAIL_SIZE;

const PAGE_SIZE: usize = 4096; //its like this in sqlite
const TABLE_MAX_PAGES: usize = 100;
const ROWS_PER_PAGE: usize = PAGE_SIZE / ROW_SIZE;

fn row_slot(table: &mut Table, row_num: usize) -> &mut [u8] {
    let page_num = row_num / ROWS_PER_PAGE;
    if table.pages[page_num].is_none() {
        table.pages[page_num] = Some(Box::new([0; PAGE_SIZE]));
    }

    let page = table.pages[page_num].as_mut().unwrap();

    let row_offset = row_num % ROWS_PER_PAGE;
    let byte_offset = row_offset * ROW_SIZE as usize;

    &mut page[byte_offset..(byte_offset + (ROW_SIZE as usize))]
}

fn execute_statement(statement: Statement, table: &mut Table) {
    match statement.statement_type {
        StatementType::Insert => {
            execute_insert(&statement, table);
        }
        StatementType::Select => {
            execute_select(&statement, table);
        }
    }
}

fn serialize_row(source: &Row, destination: &mut [u8]) {
    destination[ID_OFFSET..USERNAME_OFFSET].copy_from_slice(&source.id.to_le_bytes());
    destination[USERNAME_OFFSET..EMAIL_OFFSET].copy_from_slice(&source.username);
    destination[EMAIL_OFFSET..ROW_SIZE].copy_from_slice(&source.email);
}

fn deserialize_row(source: &[u8], destination: &mut Row) {
    destination.id = u32::from_le_bytes(source[ID_OFFSET..USERNAME_OFFSET].try_into().unwrap());
    destination.username = source[USERNAME_OFFSET..EMAIL_OFFSET].try_into().unwrap();
    destination.email = source[EMAIL_OFFSET..ROW_SIZE].try_into().unwrap();
}

fn fixed_bytes<const N: usize>(input: &str) -> Result<[u8; N], ()> {
    let bytes = input.as_bytes();
    if bytes.len() > N {
        return Err(());
    }

    let mut buffer = [0; N];
    buffer[..bytes.len()].copy_from_slice(bytes);
    Ok(buffer)
}

fn execute_insert(statement: &Statement, table: &mut Table) -> Result<(), ()> {
    if table.num_rows >= (ROWS_PER_PAGE * TABLE_MAX_PAGES) as u32 {
        return Err(());
    }

    let row_to_insert = &statement.row_to_insert;
    let slot = row_slot(table, table.num_rows as usize);

    serialize_row(row_to_insert, slot);
    table.num_rows += 1;
    Ok(())
}

fn print_row(row: &Row) {
    let id = row.id;
    let username = std::str::from_utf8(&row.username).unwrap();
    let email = std::str::from_utf8(&row.email).unwrap();
    println!("{} {} {}", id, username, email)
}

fn execute_select(statement: &Statement, table: &mut Table) -> Result<(), ()> {
    let mut row: Row = Row {
        id: 0,
        username: [0; USERNAME_SIZE],
        email: [0; EMAIL_SIZE],
    };
    for i in 0..table.num_rows {
        deserialize_row(row_slot(table, i as usize), &mut row);
        print_row(&row);
    }

    Ok(())
}

fn new_table() -> Box<Table> {
    Box::new(Table {
        num_rows: 0,
        pages: std::array::from_fn(|_| None),
    })
}
