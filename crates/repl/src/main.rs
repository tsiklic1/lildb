use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, Read, Seek, Write},
    process::exit,
};

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

        let parsed_id = id.parse::<i32>().map_err(|_| ())?;

        if parsed_id < 0 {
            return Err(());
        }

        let username = parts.next().ok_or(())?;
        let email = parts.next().ok_or(())?;

        let statement = Statement {
            statement_type: StatementType::Insert,
            row_to_insert: Row {
                id: parsed_id as u32,
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
    pager: Pager,
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

    let page = get_page(&mut table.pager, page_num).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });

    let row_offset = row_num % ROWS_PER_PAGE;
    let byte_offset = row_offset * ROW_SIZE as usize;

    &mut page[byte_offset..(byte_offset + (ROW_SIZE as usize))]
}

fn execute_statement(statement: Statement, table: &mut Table) {
    match statement.statement_type {
        StatementType::Insert => {
            if execute_insert(&statement, table).is_err() {
                println!("Error: Table full.");
            }
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

fn db_open(filename: &str) -> Box<Table> {
    let pager = match pager_open(filename) {
        Ok(pager) => pager,
        Err(()) => {
            println!("Unable to open pager");
            exit(1);
        }
    };
    let num_rows = pager.file_length / ROW_SIZE as u32;

    let table = Table { num_rows, pager };

    Box::new(table)
}

struct Pager {
    file: File,
    file_length: u32,
    pages: [Option<Box<[u8; PAGE_SIZE]>>; TABLE_MAX_PAGES],
}

fn pager_open(filename: &str) -> Result<Pager, ()> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filename)
        .map_err(|_| ())?;

    let file_length = file.metadata().map_err(|_| ())?.len() as u32;

    let pages = std::array::from_fn(|_| None);

    Ok(Pager {
        file,
        file_length,
        pages,
    })
}

fn get_page(pager: &mut Pager, page_num: usize) -> Result<&mut [u8; PAGE_SIZE], ()> {
    if page_num >= TABLE_MAX_PAGES {
        return Err(());
    }

    if pager.pages[page_num].is_none() {
        //Cache miss. Allocate memory and load from file.
        let mut page = Box::new([0; PAGE_SIZE]);
        let mut num_pages = pager.file_length / PAGE_SIZE as u32;

        // We might save a partial page at the end of the file
        if pager.file_length % PAGE_SIZE as u32 != 0 {
            num_pages += 1;
        }

        if page_num < num_pages as usize {
            pager
                .file
                .seek(io::SeekFrom::Start((page_num * PAGE_SIZE) as u64))
                .map_err(|_| ())?;

            pager.file.read(&mut page[..]).map_err(|_| ())?;
        }

        pager.pages[page_num] = Some(page);
    }

    Ok(pager.pages[page_num].as_mut().unwrap())
}

fn db_close(table: &mut Table) {
    let num_full_pages = table.num_rows / ROWS_PER_PAGE as u32;

    for i in 0..num_full_pages {
        if table.pager.pages[i as usize].is_some() {
            pager_flush(&mut table.pager, i, PAGE_SIZE as u32);
        }
    }

    let num_additional_rows = table.num_rows % ROWS_PER_PAGE as u32;
    if num_additional_rows > 0 {
        let page_num = num_full_pages;
        if table.pager.pages[page_num as usize].is_some() {
            pager_flush(
                &mut table.pager,
                page_num,
                num_additional_rows * ROW_SIZE as u32,
            );
        }
    }
}

fn pager_flush(pager: &mut Pager, page_num: u32, size: u32) -> Result<(), ()> {
    if pager.pages[page_num as usize].is_none() {
        println!("Tried to flush null page");
        return Err(());
    };

    pager
        .file
        .seek(io::SeekFrom::Start(page_num as u64 * PAGE_SIZE as u64))
        .map_err(|_| ())?;

    pager
        .file
        .write_all(&pager.pages[page_num as usize].as_ref().unwrap()[..size as usize])
        .map_err(|_| ())?;

    Ok(())
}
