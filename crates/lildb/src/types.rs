use std::fs::File;

use crate::constants::*;

pub(crate) enum StatementType {
    Insert,
    Select,
}

pub(crate) struct Statement {
    pub(crate) statement_type: StatementType,
    pub(crate) row_to_insert: Row,
}

pub(crate) struct Row {
    pub(crate) id: u32,
    pub(crate) username: [u8; USERNAME_SIZE],
    pub(crate) email: [u8; EMAIL_SIZE],
}

pub(crate) struct Table {
    pub(crate) pager: Pager,
    pub(crate) root_page_num: u32,
}

pub(crate) struct Pager {
    pub(crate) file: File,
    pub(crate) file_length: u32,
    pub(crate) num_pages: u32,
    pub(crate) pages: [Option<Box<[u8; PAGE_SIZE]>>; TABLE_MAX_PAGES],
}

pub(crate) struct Cursor<'a> {
    pub(crate) table: &'a mut Table,
    pub(crate) end_of_table: bool,
    pub(crate) page_num: u32,
    pub(crate) cell_num: u32,
}

#[derive(PartialEq)]
pub(crate) enum NodeType {
    LeafNode,
    InternalNode,
    RootNode,
}
