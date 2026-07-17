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
                ".btree" => {
                    println!("Tree:");
                    print_tree(&mut table.pager, table.root_page_num, 0);
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

// Internal Node Header Layout
const INTERNAL_NODE_NUM_KEYS_SIZE: usize = std::mem::size_of::<u32>();
const INTERNAL_NODE_NUM_KEYS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
const INTERNAL_NODE_RIGHT_CHILD_SIZE: usize = std::mem::size_of::<u32>();
const INTERNAL_NODE_RIGHT_CHILD_OFFSET: usize =
    INTERNAL_NODE_NUM_KEYS_OFFSET + INTERNAL_NODE_NUM_KEYS_SIZE;
const INTERNAL_NODE_HEADER_SIZE: usize =
    COMMON_NODE_HEADER_SIZE + INTERNAL_NODE_NUM_KEYS_SIZE + INTERNAL_NODE_RIGHT_CHILD_SIZE;

// Internal Node Body Layout
const INTERNAL_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
const INTERNAL_NODE_CHILD_SIZE: usize = std::mem::size_of::<u32>();
const INTERNAL_NODE_CELL_SIZE: usize = INTERNAL_NODE_CHILD_SIZE + INTERNAL_NODE_KEY_SIZE;
const INTENRAL_NODE_MAX_CELLS: usize = 3;

// Leaf Node Header Layout

const LEAF_NODE_NUM_CELLS_SIZE: usize = std::mem::size_of::<u32>();
const LEAF_NODE_NUM_CELLS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
const LEAF_NODE_NEXT_LEAF_SIZE: usize = std::mem::size_of::<u32>();
const LEAF_NODE_NEXT_LEAF_OFFSET: usize = LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE;
const LEAF_NODE_HEADER_SIZE: usize =
    COMMON_NODE_HEADER_SIZE + LEAF_NODE_NUM_CELLS_SIZE + LEAF_NODE_NEXT_LEAF_SIZE;

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

const INVALID_PAGE_NUM: u32 = u32::MAX;

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
        set_node_root(root_node, true);
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
    let page_num = {
        let cursor = table_find(table, 0);
        cursor.page_num
    };

    let num_cells = {
        let node = get_page(&mut table.pager, page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });

        let num_cells = leaf_node_num_cells(node);
        num_cells
    };
    let mut cursor = table_find(table, 0);
    cursor.end_of_table = num_cells == 0;
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
        let next_page_num = leaf_node_next_leaf(node);
        if next_page_num == 0 {
            // This was rightmost leaf
            cursor.end_of_table = true;
        } else {
            cursor.page_num = next_page_num;
            cursor.cell_num = 0;
        }
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
    set_node_root(node, false);
    set_leaf_node_num_cells(node, 0);
    set_leaf_node_next_leaf(node, 0); // 0 represents no sibling
}

fn leaf_node_insert(cursor: &mut Cursor, key: u32, value: &Row) {
    let node = get_page(&mut cursor.table.pager, cursor.page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    let num_cells = leaf_node_num_cells(node);

    if num_cells >= LEAF_NODE_MAX_CELLS as u32 {
        //Node full
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
        return internal_node_find(table, root_page_num, key);
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
    let old_node_copy = {
        let old_node =
            get_page(&mut cursor.table.pager, cursor.page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist.");
                exit(1);
            });
        old_node.clone()
    };
    let old_max = get_node_max_key(&old_node_copy);
    let old_node_parent = node_parent(&old_node_copy);
    {
        let new_node =
            get_page(&mut cursor.table.pager, new_page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist.");
                exit(1);
            });
        initialize_leaf_node(new_node);
        set_node_parent(new_node, old_node_parent);
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

            set_leaf_node_key(destination_node, index_within_node as u32, key);
            let destination = leaf_node_value_mut(destination_node, index_within_node as u32);
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

            let destination = leaf_node_cell_mut(destination_node, index_within_node as u32);
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
            let parent_page_num = node_parent(old_node);
            let new_max = get_node_max_key(old_node);
            let parent = get_page(&mut cursor.table.pager, parent_page_num as usize)
                .unwrap_or_else(|_| {
                    println!("Page doesn't exist");
                    exit(1);
                });

            update_internal_node_key(parent, old_max, new_max);
            internal_node_insert(cursor.table, parent_page_num, new_page_num);
        }
    }
}

fn get_unused_page_num(pager: &Pager) -> u32 {
    pager.num_pages
}

fn is_node_root(node: &[u8]) -> bool {
    node[IS_ROOT_OFFSET] != 0
}

fn create_new_root(table: &mut Table, right_child_page_num: u32) {
    // Handle splitting the root.
    // Old root copied to new page, becomes left child.
    // Address of right child passed in.
    // Re-initialize root page to contain the new root node.
    // New root node points to two children.
    let root_page_num = table.root_page_num;

    let root_copy = {
        let root = get_page(&mut table.pager, table.root_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist.");
            exit(1);
        });
        root.clone()
    };
    let right_child =
        get_page(&mut table.pager, right_child_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });
    set_node_parent(right_child, table.root_page_num);
    let left_child_page_num = get_unused_page_num(&table.pager);
    {
        // Left child has data copied from old root
        let left_child =
            get_page(&mut table.pager, left_child_page_num as usize).unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            });
        left_child.copy_from_slice(&root_copy);
        set_node_parent(left_child, table.root_page_num);
        set_node_root(left_child, false);
    }
    {
        let root = get_page(&mut table.pager, root_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });
        initialize_internal_node(root);
        set_node_root(root, true);
        set_internal_node_num_keys(root, 1);
        set_internal_node_child(root, 0, left_child_page_num);
        set_internal_node_key(root, 0, get_node_max_key(&root_copy));
        set_internal_node_right_child(root, right_child_page_num);
    }
}

fn set_node_root(node: &mut [u8], is_root: bool) {
    let value: u8 = match is_root {
        true => 1,
        false => 0,
    };

    node[IS_ROOT_OFFSET] = value;
}

fn initialize_internal_node(node: &mut [u8]) {
    set_node_type(node, NodeType::InternalNode);
    set_node_root(node, false);
    set_internal_node_num_keys(node, 0);
}

fn internal_node_num_keys(node: &[u8]) -> u32 {
    let start = INTERNAL_NODE_NUM_KEYS_OFFSET;
    let end = start + INTERNAL_NODE_NUM_KEYS_SIZE;
    let bytes: [u8; 4] = node[start..end].try_into().unwrap();
    let value = u32::from_le_bytes(bytes);
    value
}

fn set_internal_node_num_keys(node: &mut [u8], num_keys: u32) {
    let start = INTERNAL_NODE_NUM_KEYS_OFFSET;
    let end = start + INTERNAL_NODE_NUM_KEYS_SIZE;
    let bytes = num_keys.to_le_bytes();
    node[start..end].copy_from_slice(&bytes);
}

fn internal_node_child(node: &[u8], child_num: u32) -> u32 {
    let num_keys = internal_node_num_keys(node);
    if child_num > num_keys {
        println!(
            "Tried to access child_num {} > num_keys {}",
            child_num, num_keys
        );
        exit(1);
    }
    let child_bytes = if child_num == num_keys {
        internal_node_right_child(node)
    } else {
        let cell = internal_node_cell(node, child_num);
        &cell[0..INTERNAL_NODE_CHILD_SIZE]
    };

    u32::from_le_bytes(child_bytes.try_into().unwrap())
}

fn set_internal_node_child(node: &mut [u8], child_num: u32, value: u32) {
    let num_keys = internal_node_num_keys(node);
    if child_num > num_keys {
        println!(
            "Tried to access child_num {} > num_keys {}",
            child_num, num_keys
        );
        exit(1);
    };
    let child_bytes = if child_num == num_keys {
        internal_node_right_child_mut(node)
    } else {
        let cell = internal_node_cell_mut(node, child_num);
        &mut cell[0..INTERNAL_NODE_CHILD_SIZE]
    };

    let value_bytes = value.to_le_bytes();
    child_bytes.copy_from_slice(&value_bytes);
}

fn internal_node_right_child(node: &[u8]) -> &[u8] {
    let start = INTERNAL_NODE_RIGHT_CHILD_OFFSET;
    let end = start + INTERNAL_NODE_RIGHT_CHILD_SIZE;
    &node[start..end]
}

fn set_internal_node_right_child(node: &mut [u8], value: u32) {
    let start = INTERNAL_NODE_RIGHT_CHILD_OFFSET;
    let end = start + INTERNAL_NODE_RIGHT_CHILD_SIZE;
    let value_bytes = value.to_le_bytes();
    node[start..end].copy_from_slice(&value_bytes);
}

fn internal_node_right_child_mut(node: &mut [u8]) -> &mut [u8] {
    let start = INTERNAL_NODE_RIGHT_CHILD_OFFSET;
    let end = start + INTERNAL_NODE_RIGHT_CHILD_SIZE;
    &mut node[start..end]
}

fn internal_node_cell(node: &[u8], cell_num: u32) -> &[u8] {
    let start = INTERNAL_NODE_HEADER_SIZE + cell_num as usize * INTERNAL_NODE_CELL_SIZE;
    let end = start + INTERNAL_NODE_CELL_SIZE;
    &node[start..end]
}

fn internal_node_cell_mut(node: &mut [u8], cell_num: u32) -> &mut [u8] {
    let start = INTERNAL_NODE_HEADER_SIZE + cell_num as usize * INTERNAL_NODE_CELL_SIZE;
    let end = start + INTERNAL_NODE_CELL_SIZE;
    &mut node[start..end]
}

fn internal_node_key(node: &[u8], key_num: u32) -> u32 {
    let cell = internal_node_cell(node, key_num);
    let start = INTERNAL_NODE_CHILD_SIZE;
    let end = start + INTERNAL_NODE_KEY_SIZE;

    u32::from_le_bytes(cell[start..end].try_into().unwrap())
}

fn set_internal_node_key(node: &mut [u8], key_num: u32, value: u32) {
    let cell = internal_node_cell_mut(node, key_num);
    let start = INTERNAL_NODE_CHILD_SIZE;
    let end = start + INTERNAL_NODE_KEY_SIZE;
    let value_bytes = value.to_le_bytes();

    cell[start..end].copy_from_slice(&value_bytes);
}

// fn get_node_max_key(node: &[u8]) -> u32 {
//     let node_type = get_node_type(node);
//     match node_type {
//         NodeType::InternalNode => {
//             let num_keys = internal_node_num_keys(node);
//             internal_node_key(node, num_keys - 1)
//         }
//         NodeType::LeafNode => {
//             let num_keys = leaf_node_num_cells(node);
//             leaf_node_key(node, num_keys - 1)
//         }
//         _ => exit(1),
//     }
// }

fn indent(level: u32) {
    for _ in 0..level {
        print!("    ");
    }
}

fn print_tree(pager: &mut Pager, page_num: u32, indentation_level: u32) {
    enum TreeNodeInfo {
        Leaf { keys: Vec<u32> },
        Internal { keys: Vec<u32>, children: Vec<u32> },
    }

    let info = {
        let node = get_page(pager, page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });

        match get_node_type(node) {
            NodeType::LeafNode => {
                let num_keys = leaf_node_num_cells(node);
                let mut keys = Vec::new();

                for i in 0..num_keys {
                    keys.push(leaf_node_key(node, i));
                }

                TreeNodeInfo::Leaf { keys }
            }
            NodeType::InternalNode => {
                let num_keys = internal_node_num_keys(node);
                let mut keys = Vec::new();
                let mut children = Vec::new();

                for i in 0..num_keys {
                    children.push(internal_node_child(node, i));
                    keys.push(internal_node_key(node, i));
                }

                children.push(internal_node_child(node, num_keys));

                TreeNodeInfo::Internal { keys, children }
            }
            _ => exit(1),
        }
    };

    match info {
        TreeNodeInfo::Leaf { keys } => {
            indent(indentation_level);
            println!("- leaf (size {})", keys.len());

            for key in keys {
                indent(indentation_level + 1);
                println!("- {}", key);
            }
        }
        TreeNodeInfo::Internal { keys, children } => {
            indent(indentation_level);
            println!("- internal (size {})", keys.len());

            for i in 0..keys.len() {
                print_tree(pager, children[i], indentation_level + 1);

                indent(indentation_level + 1);
                println!("- key {}", keys[i]);
            }

            print_tree(pager, children[keys.len()], indentation_level + 1);
        }
    }
}

fn internal_node_find(table: &mut Table, page_num: u32, key: u32) -> Cursor {
    let node = get_page(&mut table.pager, page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    // let num_keys = internal_node_num_keys(node);

    // // Binary search
    // let mut min_index = 0;
    // let mut max_index = num_keys; // there is one more child than key

    // while min_index != max_index {
    //     let index = (min_index + max_index) / 2;
    //     let key_to_right = internal_node_key(node, index);
    //     if key_to_right >= key {
    //         max_index = index;
    //     } else {
    //         min_index = index + 1;
    //     }
    // }

    let child_index = internal_node_find_child(node, key);
    let child_num = internal_node_child(node, child_index);

    let child = get_page(&mut table.pager, child_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });

    let node_type = get_node_type(child);

    let cursor = match node_type {
        NodeType::LeafNode => {
            let leaf_node = leaf_node_find(table, child_num, key);
            leaf_node
        }
        NodeType::InternalNode => {
            let internal_node = internal_node_find(table, child_num, key);
            internal_node
        }
        _ => exit(1),
    };

    cursor
}

fn leaf_node_next_leaf(node: &[u8]) -> u32 {
    let start = LEAF_NODE_NEXT_LEAF_OFFSET;
    let end = start + LEAF_NODE_NEXT_LEAF_SIZE;
    u32::from_le_bytes(node[start..end].try_into().unwrap())
}

fn set_leaf_node_next_leaf(node: &mut [u8], value: u32) {
    let start = LEAF_NODE_NEXT_LEAF_OFFSET;
    let end = start + LEAF_NODE_NEXT_LEAF_SIZE;
    let bytes = value.to_le_bytes();
    node[start..end].copy_from_slice(&bytes);
}

fn node_parent(node: &[u8]) -> u32 {
    let start = PARENT_POINTER_OFFSET;
    let end = start + PARENT_POINTER_SIZE;
    u32::from_le_bytes(node[start..end].try_into().unwrap())
}

fn set_node_parent(node: &mut [u8], value: u32) {
    let start = PARENT_POINTER_OFFSET;
    let end = start + PARENT_POINTER_SIZE;
    let bytes = value.to_le_bytes();
    node[start..end].copy_from_slice(&bytes);
}

fn update_internal_node_key(node: &mut [u8], old_key: u32, new_key: u32) {
    let old_child_index = internal_node_find_child(node, old_key);
    set_internal_node_key(node, old_child_index, new_key);
}

fn internal_node_find_child(node: &[u8], key: u32) -> u32 {
    // Return the index of the child which should contain the given key.
    let num_keys = internal_node_num_keys(node);

    // Binary search
    let mut min_index = 0;
    let mut max_index = num_keys; // there is one more child than key

    while min_index != max_index {
        let index = (min_index + max_index) / 2;
        let key_to_right = internal_node_key(node, index);
        if key_to_right >= key {
            max_index = index;
        } else {
            min_index = index + 1;
        }
    }

    min_index
}

fn internal_node_insert(table: &mut Table, parent_page_num: u32, child_page_num: u32) {
    // Add a new child/key pair to parent that corresponds to child

    let mut parent = {
        let parent = get_page(&mut table.pager, parent_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });
        parent.clone()
    };
    let child = get_page(&mut table.pager, child_page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    let child_max_key = get_node_max_key(child);
    let index = internal_node_find_child(&parent, child_max_key);

    let original_num_keys = internal_node_num_keys(&parent);
    set_internal_node_num_keys(&mut parent, original_num_keys + 1);

    if original_num_keys >= INTENRAL_NODE_MAX_CELLS as u32 {
        println!("Need to implement splitting ");
        exit(1);
    }

    let right_child_page_num =
        u32::from_le_bytes(internal_node_right_child(&parent).try_into().unwrap());
    let right_child =
        get_page(&mut table.pager, right_child_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1)
        });

    if child_max_key > get_node_max_key(right_child) {
        // Replace right child
        set_internal_node_child(&mut parent, original_num_keys, right_child_page_num);
        set_internal_node_key(
            &mut parent,
            original_num_keys,
            get_node_max_key(right_child),
        );
        set_internal_node_right_child(&mut parent, child_page_num);
    } else {
        // Make room for the new cell
        for i in (index..=original_num_keys).rev() {
            let source = internal_node_cell(&parent, i - 1).to_vec();
            let destination = internal_node_cell_mut(&mut parent, i);
            destination.copy_from_slice(&source);
        }
        set_internal_node_child(&mut parent, index, child_page_num);
        set_internal_node_key(&mut parent, index, child_max_key);
    }

    let parent_page = get_page(&mut table.pager, parent_page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    parent_page.copy_from_slice(&parent);
}

fn internal_node_split_and_insert(table: &mut Table, parent_page_num: u32, child_page_num: u32) {
    let mut old_page_num = parent_page_num;
    let mut old_node = get_page(&mut table.pager, parent_page_num as usize)
        .unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        })
        .to_vec();
    let old_max = get_node_max_key(&old_node);

    let child = get_page(&mut table.pager, child_page_num as usize).unwrap_or_else(|_| {
        println!("Page doesn't exist");
        exit(1);
    });
    let child_max = get_node_max_key(child);

    let new_page_num = get_unused_page_num(&table.pager);

    // Declaring a flag before updating pointers which
    // records whether this operation involves splitting the root -
    // if it does, we will insert our newly created node during
    // the step where the table's new root is created. If it does
    // not, we have to insert the newly created node into its parent
    // after the old node's keys have been transferred over. We are not
    // able to do this if the newly created node's parent is not a newly
    // initialized root node, because in that case its parent may have existing
    // keys aside from our old node which we are splitting. If that is true, we
    // need to find a place for our newly created node in its parent, and we
    // cannot insert it at the correct index if it does not yet have any keys

    let splitting_root = is_node_root(&old_node);

    let parent = if splitting_root {
        create_new_root(table, new_page_num);
        let parent = get_page(&mut table.pager, table.root_page_num as usize)
            .unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            })
            .to_vec();

        // If we are splitting the root, we need to update old_node to point
        // to the new root's left child, new_page_num will already point to
        // the new root's right child

        old_page_num = internal_node_child(&parent, 0);
        old_node = get_page(&mut table.pager, old_page_num as usize)
            .unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            })
            .to_vec();
        parent
    } else {
        let old_node_parent = node_parent(&old_node);
        let parent = get_page(&mut table.pager, old_node_parent as usize)
            .unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            })
            .to_vec();
        let new_node = get_page(&mut table.pager, new_page_num as usize).unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        });
        initialize_internal_node(new_node);
        parent
    };

    let mut old_num_keys = internal_node_num_keys(&old_node);
    let mut cur_page_num =
        u32::from_le_bytes(internal_node_right_child(&old_node).try_into().unwrap());
    let mut cur = get_page(&mut table.pager, cur_page_num as usize)
        .unwrap_or_else(|_| {
            println!("Page doesn't exist");
            exit(1);
        })
        .to_vec();

    // First put right child into new node and set right child of old node to invalid page number
    internal_node_insert(table, new_page_num, cur_page_num);
    set_node_parent(&mut cur, new_page_num);

    set_internal_node_right_child(&mut old_node, INVALID_PAGE_NUM);

    // For each key until you get to the middle key, move the key and the child to the new node
    for i in ((INTENRAL_NODE_MAX_CELLS / 2)..INTENRAL_NODE_MAX_CELLS).rev() {
        cur_page_num = internal_node_child(&old_node, i as u32);
        cur = get_page(&mut table.pager, cur_page_num as usize)
            .unwrap_or_else(|_| {
                println!("Page doesn't exist");
                exit(1);
            })
            .to_vec();
        internal_node_insert(table, new_page_num, cur_page_num);
        set_node_parent(&mut cur, new_page_num);

        old_num_keys -= 1;
    }

    // Set child before middle key, which is now the highest key, to be node's right child,
    // and decrement number of keys

    let right_child = internal_node_child(&old_node, old_num_keys - 1);
    set_internal_node_right_child(&mut old_node, right_child);

    // Determine which of the two nodes after the split should contain the child to be inserted,
    // and insert the child

    let max_after_split = get_node_max_key();
}

fn get_node_max_key(mut pager: Pager, node: &[u8]) -> u32 {
    let node_type = get_node_type(node);
    if node_type == NodeType::LeafNode {
        return leaf_node_key(node, leaf_node_num_cells(node));
    }
    let child = u32::from_le_bytes(internal_node_right_child(node).try_into().unwrap());
    let right_child = get_page(&mut pager, child as usize);
    return get_node_max_key(pager, node);
}
