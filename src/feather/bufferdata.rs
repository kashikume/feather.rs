 pub(crate) struct BufferData {
    pub buffer_id: u64,
    pub vertex_begin_index: u64,
    pub vertex_size: u64,
    pub index_begin_index: u64,
    pub index_size: u64,
}

impl BufferData {
    pub fn new(buffer_id: u64, vertex_begin_index: u64, vertex_size: u64, index_begin_index: u64, index_size: u64) -> Self {
        Self { 
            buffer_id, 
            vertex_begin_index, 
            vertex_size, 
            index_begin_index, 
            index_size 
        }
    }

    pub fn end_vertex(&self) -> u64 {
        self.vertex_begin_index + self.vertex_size
    }

    pub fn end_index(&self) -> u64 {
        self.index_begin_index + self.index_size
    }

    pub fn last_vertex(&self) -> u64 {
        self.end_vertex() - 1
    }

    pub fn last_index(&self) -> u64 {
        self.end_index() - 1
    }
}