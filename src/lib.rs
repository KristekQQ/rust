pub mod graphics;
pub mod input;

#[cfg(test)]
mod tests {
    use crate::graphics::vertex::{INDICES, VERTICES};

    #[test]
    fn cube_vertex_count() {
        assert_eq!(VERTICES.len(), 24);
    }

    #[test]
    fn cube_index_count() {
        assert_eq!(INDICES.len(), 36);
    }
}

