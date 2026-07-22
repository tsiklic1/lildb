pub(crate) const ID_SIZE: usize = 4;
pub(crate) const USERNAME_SIZE: usize = 32;
pub(crate) const EMAIL_SIZE: usize = 255;
pub(crate) const ID_OFFSET: usize = 0;
pub(crate) const USERNAME_OFFSET: usize = ID_OFFSET + ID_SIZE;
pub(crate) const EMAIL_OFFSET: usize = USERNAME_OFFSET + USERNAME_SIZE;
pub(crate) const ROW_SIZE: usize = ID_SIZE + USERNAME_SIZE + EMAIL_SIZE;

pub(crate) const PAGE_SIZE: usize = 4096; //its like this in sqlite
pub(crate) const TABLE_MAX_PAGES: usize = 100;

pub(crate) const NODE_TYPE_SIZE: usize = std::mem::size_of::<u8>();
pub(crate) const NODE_TYPE_OFFSET: usize = 0;
pub(crate) const IS_ROOT_SIZE: usize = std::mem::size_of::<u8>();
pub(crate) const IS_ROOT_OFFSET: usize = NODE_TYPE_SIZE;
pub(crate) const PARENT_POINTER_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const PARENT_POINTER_OFFSET: usize = IS_ROOT_OFFSET + IS_ROOT_SIZE;
pub(crate) const COMMON_NODE_HEADER_SIZE: usize = NODE_TYPE_SIZE + IS_ROOT_SIZE + PARENT_POINTER_SIZE;

// Internal Node Header Layout
pub(crate) const INTERNAL_NODE_NUM_KEYS_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const INTERNAL_NODE_NUM_KEYS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
pub(crate) const INTERNAL_NODE_RIGHT_CHILD_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const INTERNAL_NODE_RIGHT_CHILD_OFFSET: usize =
    INTERNAL_NODE_NUM_KEYS_OFFSET + INTERNAL_NODE_NUM_KEYS_SIZE;
pub(crate) const INTERNAL_NODE_HEADER_SIZE: usize =
    COMMON_NODE_HEADER_SIZE + INTERNAL_NODE_NUM_KEYS_SIZE + INTERNAL_NODE_RIGHT_CHILD_SIZE;

// Internal Node Body Layout
pub(crate) const INTERNAL_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const INTERNAL_NODE_CHILD_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const INTERNAL_NODE_CELL_SIZE: usize = INTERNAL_NODE_CHILD_SIZE + INTERNAL_NODE_KEY_SIZE;
pub(crate) const INTERNAL_NODE_MAX_CELLS: usize = 3;

// Leaf Node Header Layout

pub(crate) const LEAF_NODE_NUM_CELLS_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const LEAF_NODE_NUM_CELLS_OFFSET: usize = COMMON_NODE_HEADER_SIZE;
pub(crate) const LEAF_NODE_NEXT_LEAF_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const LEAF_NODE_NEXT_LEAF_OFFSET: usize = LEAF_NODE_NUM_CELLS_OFFSET + LEAF_NODE_NUM_CELLS_SIZE;
pub(crate) const LEAF_NODE_HEADER_SIZE: usize =
    COMMON_NODE_HEADER_SIZE + LEAF_NODE_NUM_CELLS_SIZE + LEAF_NODE_NEXT_LEAF_SIZE;

// Leaf Node Body Layout
pub(crate) const LEAF_NODE_KEY_SIZE: usize = std::mem::size_of::<u32>();
pub(crate) const LEAF_NODE_KEY_OFFSET: usize = 0;
pub(crate) const LEAF_NODE_VALUE_SIZE: usize = ROW_SIZE;
pub(crate) const LEAF_NODE_VALUE_OFFSET: usize = LEAF_NODE_KEY_OFFSET + LEAF_NODE_KEY_SIZE;
pub(crate) const LEAF_NODE_CELL_SIZE: usize = LEAF_NODE_KEY_SIZE + LEAF_NODE_VALUE_SIZE;
pub(crate) const LEAF_NODE_SPACE_FOR_CELLS: usize = PAGE_SIZE - LEAF_NODE_HEADER_SIZE;
pub(crate) const LEAF_NODE_MAX_CELLS: usize = LEAF_NODE_SPACE_FOR_CELLS / LEAF_NODE_CELL_SIZE;

pub(crate) const LEAF_NODE_RIGHT_SPLIT_COUNT: usize = (LEAF_NODE_MAX_CELLS + 1) / 2;
pub(crate) const LEAF_NODE_LEFT_SPLIT_COUNT: usize = LEAF_NODE_MAX_CELLS + 1 - LEAF_NODE_RIGHT_SPLIT_COUNT;

pub(crate) const INVALID_PAGE_NUM: u32 = u32::MAX;
