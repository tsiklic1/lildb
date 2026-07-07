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
                ".constants" => {
                    println!("Constants: ");
                    print_constants();
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
    pager: Pager,
    root_page_num: u32,
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

const NODE_TYPE_SIZE: usize = std::mem::size_of::<u8>();
const NODE_TYPE_OFFSET: usize = 0;
const IS_ROOT_SIZE: usize = std::mem::size_of::<u8>();
const IS_ROOT_OFFSET: usize = NODE_TYPE_SIZE;
const PARENT_POINTER_SIZE: usize = std::mem::size_of::<u32>();
const PARENT_POINTER_OFFSET: usize = IS_ROOT_OFFSET + IS_ROOT_SIZE;
const COMMON_NODE_HEADER_SIZE: usize = NODE_TYPE_SIZE + IS_ROOT_SIZE + PARENT_POINTER_SIZE;

// Leaf Node Header Layout

const LEAF_NODE_NUM_CELLS_SIZE: usize = std::mem::size_of::<u32>();
const LEAF_NODE_NUM_CELLS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
const LEAF_NODE_HEADER_SIZE: usize = COMMON_NODE_HEADER_SIZE + LEAF_NODE_NUM_CELLS_SIZE;

// Leaf Node Body Layout
const LEAF_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
const LEAF_NODE_KEY_OFFSET: usize = 0;
const LEAF_NODE_VALUE_SIZE: usize = ROW_SIZE;
const LEAF_NODE_VALUE_OFFSET: usize = LEAF_NODE_KEY_OFFSET + LEAF_NODE_KEY_SIZE;
const LEAF_NODE_CELL_SIZE: usize = LEAF_NODE_KEY_SIZE + LEAF_NODE_VALUE_SIZE;
const LEAF_NODE_SPACE_FOR_CELLS: usize = PAGE_SIZE - LEAF_NODE_HEADER_SIZE;
const LEAF_NODE_MAX_CELLS: usize = LEAF_NODE_SPACE_FOR_CELLS / LEAF_NODE_CELL_SIZE;

const LEAF_NODE_RIGHT_SPLIT_COUNT: usize = (LEAF_NODE_MAX_CELLS + 1) / 2;
const LEAF_NODE_LEFT_SPLIT_COUNT: usize = LEAF_NODE_MAX_CELLS + 1 - LEAF_NODE_RIGHT_SPLIT_COUNT;

fn cursor_value<'cursor, 'table>(cursor: &'cursor mut Cursor<'table>) -> &'cursor mut [u8] {
    let page_num = cursor.page_num;

    let page = get_page(&mut cursor.table.pager, page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });

    leaf_node_value_mut(page, cursor.cell_num)
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
    let num_cells = {
        let node = get_page(&mut table.pager, table.root_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exits");
            exit(1);
        });

        let num_cells = leaf_node_num_cells(node);

        num_cells
    };

    let row_to_insert = &statement.row_to_insert;

    let key_to_insert = row_to_insert.id;
    let mut cursor = table_find(table, key_to_insert);

    if cursor.cell_num < num_cells {
        let node =
            get_page(&mut cursor.table.pager, cursor.page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exits");
                exit(1);
            });
        let key_at_index = leaf_node_key(node, cursor.cell_num);
        if key_at_index == key_to_insert {
            return Err(());
        }
    }

    leaf_node_insert(&mut cursor, row_to_insert.id, row_to_insert);

    Ok(())
}

fn print_row(row: &Row) {
    let id = row.id;
    let username = std::str::from_utf8(&row.username).unwrap();
    let email = std::str::from_utf8(&row.email).unwrap();
    println!("{} {} {}", id, username, email)
}

fn execute_select(_statement: &Statement, table: &mut Table) -> Result<(), ()> {
    let mut cursor = table_start(table);
    let mut row: Row = Row {
        id: 0,
        username: [0; USERNAME_SIZE],
        email: [0; EMAIL_SIZE],
    };

    while !cursor.end_of_table {
        deserialize_row(cursor_value(&mut cursor), &mut row);
        print_row(&row);
        cursor_advance(&mut cursor);
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

    let mut table = Table {
        root_page_num: 0,
        pager,
    };

    if table.pager.num_pages == 0 {
        // New database file. Initialize page 0 as leaf node.
        let root_node = get_page(&mut table.pager, 0).unwrap_or_else(|_| {
            println!("Page not found");
            exit(1);
        });
        initialize_leaf_node(root_node);
    }

    Box::new(table)
}

struct Pager {
    file: File,
    file_length: u32,
    num_pages: u32,
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

    if file_length % PAGE_SIZE as u32 != 0 {
        println!("Db file is not a whole number of pages. Corrupt file.");
        exit(1);
    }

    let pages = std::array::from_fn(|_| None);

    Ok(Pager {
        file,
        file_length,
        pages,
        num_pages: file_length / PAGE_SIZE as u32,
    })
}

fn get_page(pager: &mut Pager, page_num: usize) -> Result<&mut [u8; PAGE_SIZE], ()> {
    if page_num >= pager.num_pages as usize {
        pager.num_pages = page_num as u32 + 1;
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
    for i in 0..table.pager.num_pages {
        if table.pager.pages[i as usize].is_some() {
            pager_flush(&mut table.pager, i);
        }
    }
}

fn pager_flush(pager: &mut Pager, page_num: u32) -> Result<(), ()> {
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
        .write_all(&pager.pages[page_num as usize].as_ref().unwrap()[..PAGE_SIZE as usize])
        .map_err(|_| ())?;

    Ok(())
}

struct Cursor<'a> {
    table: &'a mut Table,
    end_of_table: bool,
    page_num: u32,
    cell_num: u32,
}

fn table_start(table: &mut Table) -> Cursor<'_> {
    let mut default = [0; 4096];
    let root_node = get_page(&mut table.pager, table.root_page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    let num_cells = leaf_node_num_cells(root_node);

    let cursor = Cursor {
        page_num: table.root_page_num,
        end_of_table: num_cells == 0,
        table,
        cell_num: 0,
    };

    cursor
}

fn table_end(table: &mut Table) -> Cursor<'_> {
    let root_node = get_page(&mut table.pager, table.root_page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    let num_cells = leaf_node_num_cells(root_node);

    let cursor = Cursor {
        page_num: table.root_page_num,
        end_of_table: true,
        table,
        cell_num: num_cells,
    };

    cursor
}

fn cursor_advance(cursor: &mut Cursor) {
    let page_num = cursor.page_num;
    let node = get_page(&mut cursor.table.pager, page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });

    cursor.cell_num += 1;

    if cursor.cell_num >= leaf_node_num_cells(node) {
        cursor.end_of_table = true;
    }
}

#[derive(PartialEq)]
enum NodeType {
    LeafNode,
    InternalNode,
    RootNode,
}

fn leaf_node_num_cells(node: &[u8]) -> u32 {
    let start = LEAF_NODE_NUM_CELLS_OFFSET;
    let end = start + LEAF_NODE_NUM_CELLS_SIZE;
    return u32::from_le_bytes(node[start..end].try_into().unwrap());
}

fn set_leaf_node_num_cells(node: &mut [u8], value: u32) {
    let start = LEAF_NODE_NUM_CELLS_OFFSET;
    let end = start + LEAF_NODE_NUM_CELLS_SIZE;
    let value_bytes = value.to_le_bytes();
    node[start..end].copy_from_slice(&value_bytes);
}

fn leaf_node_cell(node: &[u8], cell_num: u32) -> &[u8] {
    let start = LEAF_NODE_HEADER_SIZE + cell_num as usize * LEAF_NODE_CELL_SIZE;
    let end = start + LEAF_NODE_CELL_SIZE;

    return &node[start..end];
}

fn leaf_node_cell_mut(node: &mut [u8], cell_num: u32) -> &mut [u8] {
    let start = LEAF_NODE_HEADER_SIZE + cell_num as usize * LEAF_NODE_CELL_SIZE;
    let end = start + LEAF_NODE_CELL_SIZE;

    return &mut node[start..end];
}

fn leaf_node_key(node: &[u8], cell_num: u32) -> u32 {
    let start =
        LEAF_NODE_HEADER_SIZE + cell_num as usize * LEAF_NODE_CELL_SIZE + LEAF_NODE_KEY_OFFSET;
    let end = start + LEAF_NODE_KEY_SIZE;

    u32::from_le_bytes(node[start..end].try_into().unwrap())
}

fn set_leaf_node_key(node: &mut [u8], cell_num: u32, value: u32) {
    let start =
        LEAF_NODE_HEADER_SIZE + cell_num as usize * LEAF_NODE_CELL_SIZE + LEAF_NODE_KEY_OFFSET;
    let end = start + LEAF_NODE_KEY_SIZE;
    let value_bytes = value.to_le_bytes();

    node[start..end].copy_from_slice(&value_bytes);
}

fn leaf_node_value(node: &[u8], cell_num: u32) -> &[u8] {
    let start =
        LEAF_NODE_HEADER_SIZE + cell_num as usize * LEAF_NODE_CELL_SIZE + LEAF_NODE_VALUE_OFFSET;
    let end = start + LEAF_NODE_VALUE_SIZE;

    &node[start..end]
}

fn leaf_node_value_mut(node: &mut [u8], cell_num: u32) -> &mut [u8] {
    let start =
        LEAF_NODE_HEADER_SIZE + cell_num as usize * LEAF_NODE_CELL_SIZE + LEAF_NODE_VALUE_OFFSET;
    let end = start + LEAF_NODE_VALUE_SIZE;

    &mut node[start..end]
}

fn initialize_leaf_node(node: &mut [u8]) {
    set_node_type(node, NodeType::LeafNode);
    set_leaf_node_num_cells(node, 0);
}

fn leaf_node_insert(cursor: &mut Cursor, key: u32, value: &Row) {
    let node = get_page(&mut cursor.table.pager, cursor.page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    let num_cells = leaf_node_num_cells(node);

    if num_cells >= LEAF_NODE_MAX_CELLS as u32 {
        //Node full
        println!("Need to implement splitting a leaf node.");
        leaf_node_split_and_insert(cursor, key, value);
        return;
    }

    //can be optimized with copy_within()
    let node_copy: [u8; PAGE_SIZE] = node[..].try_into().unwrap();

    if cursor.cell_num < num_cells {
        // Make room for new cell
        for i in (cursor.cell_num + 1..=num_cells).rev() {
            let source = leaf_node_cell(&node_copy, i - 1);
            let destination = leaf_node_cell_mut(node, i);
            destination.copy_from_slice(source);
        }
    }

    set_leaf_node_num_cells(node, num_cells + 1);
    set_leaf_node_key(node, cursor.cell_num, key);
    let value_bytes = leaf_node_value_mut(node, cursor.cell_num);
    serialize_row(value, value_bytes);
}

fn print_constants() {
    println!("ROW_SIZE: {}", ROW_SIZE);
    println!("COMMON_NODE_HEADER_SIZE: {}", COMMON_NODE_HEADER_SIZE);
    println!("LEAF_NODE_HEADER_SIZE: {}", LEAF_NODE_HEADER_SIZE);
    println!("LEAF_NODE_CELL_SIZE: {}", LEAF_NODE_CELL_SIZE);
    println!("LEAF_NODE_SPACE_FOR_CELLS: {}", LEAF_NODE_SPACE_FOR_CELLS);
    println!("LEAF_NODE_MAX_CELLS: {}", LEAF_NODE_MAX_CELLS);
}

// Return the position of the given key.
// If the key is not present, return the position where it should be inserted
fn table_find(table: &mut Table, key: u32) -> Cursor {
    let root_page_num = table.root_page_num;
    let root_node = get_page(&mut table.pager, root_page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });

    if get_node_type(root_node) == NodeType::LeafNode {
        let cursor = leaf_node_find(table, root_page_num, key);
        return cursor;
    } else {
        println!("Need to implement searching an internal node");
        exit(1);
    }
}

fn get_node_type(node: &[u8]) -> NodeType {
    let value = node[NODE_TYPE_OFFSET];
    match value {
        0 => NodeType::InternalNode,
        1 => NodeType::LeafNode,
        2 => NodeType::RootNode,
        _ => {
            println!("Unknown node type: {}", value);
            exit(1);
        }
    }
}

fn set_node_type(node: &mut [u8], node_type: NodeType) {
    node[NODE_TYPE_OFFSET] = match node_type {
        NodeType::InternalNode => 0,
        NodeType::LeafNode => 1,
        NodeType::RootNode => 2,
    };
}

fn leaf_node_find(table: &mut Table, page_num: u32, key: u32) -> Cursor {
    let cell_num = {
        let node = get_page(&mut table.pager, page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });

        let num_cells = leaf_node_num_cells(node);
        // Binary search
        let mut min_index = 0;
        let mut one_past_max_index = num_cells;
        while one_past_max_index != min_index {
            let index = (min_index + one_past_max_index) / 2;
            let key_at_index = leaf_node_key(node, index);

            if key == key_at_index {
                return Cursor {
                    table,
                    end_of_table: false,
                    page_num,
                    cell_num: index,
                };
            }
            if key < key_at_index {
                one_past_max_index = index;
            } else {
                min_index = index + 1;
            }
        }

        min_index
    };

    Cursor {
        table,
        page_num,
        end_of_table: false,
        cell_num,
    }
}

fn leaf_node_split_and_insert(cursor: &mut Cursor, key: u32, value: &Row) {
    // Create a new node and move half the cells over.
    // Insert the new value in one of the two nodes.
    // Update parent or create a new parent.

    let old_page_num = cursor.page_num;
    let new_page_num = get_unused_page_num(&cursor.table.pager);
    let mut old_node_copy = {
        let old_node =
            get_page(&mut cursor.table.pager, cursor.page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist.");
                exit(1);
            });
        old_node.clone()
    };
    {
        let mut new_node = get_page(&mut cursor.table.pager, cursor.page_num as usize)
            .unwrap_or_else(|_| {
                println!("Page doesn't exist.");
                exit(1);
            });
        initialize_leaf_node(new_node);
    }

    // All existing keys plus new key should be divided
    // evenly between old (left) and new (right) nodes.
    // Starting from the right, move each key to correct position.
    for i in (0..=LEAF_NODE_MAX_CELLS).rev() {
        let destination_page_num = if i >= LEAF_NODE_LEFT_SPLIT_COUNT {
            new_page_num
        } else {
            old_page_num
        };

        let index_within_node = i % LEAF_NODE_LEFT_SPLIT_COUNT;

        if i == cursor.cell_num as usize {
            let destination_node = get_page(&mut cursor.table.pager, destination_page_num as usize)
                .unwrap_or_else(|_| {
                    println!("Page doesn't exist.");
                    exit(1);
                });

            let destination = leaf_node_cell_mut(destination_node, index_within_node as u32);
            serialize_row(value, destination);
        } else {
            let source_index = if i > cursor.cell_num as usize {
                i - 1
            } else {
                i
            };

            let source_cell = leaf_node_cell(&old_node_copy, source_index as u32).to_vec();
            let destination_node = get_page(&mut cursor.table.pager, destination_page_num as usize)
                .unwrap_or_else(|_| {
                    println!("Page doesn't exist");
                    exit(1);
                });

            let mut destination =
                leaf_node_cell_mut(destination_node, index_within_node as u32).to_vec();
            destination.copy_from_slice(&source_cell);
        }
    }
    {
        let old_node =
            get_page(&mut cursor.table.pager, old_page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            });
        set_leaf_node_num_cells(old_node, LEAF_NODE_LEFT_SPLIT_COUNT as u32);
    }
    {
        let new_node =
            get_page(&mut cursor.table.pager, new_page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            });
        set_leaf_node_num_cells(new_node, LEAF_NODE_RIGHT_SPLIT_COUNT as u32);
    }
    {
        let old_node =
            get_page(&mut cursor.table.pager, old_page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            });
        if is_node_root(old_node) {
            create_new_root(cursor.table, new_page_num);
        } else {
        }
    }
}

fn get_unused_page_num(pager: &Pager) -> u32 {
    pager.num_pages
}

fn is_node_root(node: &mut [u8; 4096]) -> bool {
    let node_type = get_node_type(node);
    match node_type {
        NodeType::RootNode => true,
        _ => false,
    }
}
