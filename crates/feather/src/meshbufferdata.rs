 pub(crate) struct MeshBufferData {
    pub buffer_handle: usize,
    pub vertex_begin_index: usize,
    pub vertex_size: usize,
    pub index_begin_index: usize,
    pub index_size: usize,
}

impl MeshBufferData {
    pub fn new(buffer_handle: usize, vertex_begin_index: usize, vertex_size: usize, index_begin_index: usize, index_size: usize) -> Self {
        Self { 
            buffer_handle, 
            vertex_begin_index, 
            vertex_size, 
            index_begin_index, 
            index_size 
        }
    }

    pub fn end_vertex(&self) -> usize {
        self.vertex_begin_index + self.vertex_size
    }

    pub fn end_index(&self) -> usize {
        self.index_begin_index + self.index_size
    }

    pub fn last_vertex(&self) -> usize {
        self.end_vertex() - 1
    }

    pub fn last_index(&self) -> usize {
        self.end_index() - 1
    }
}