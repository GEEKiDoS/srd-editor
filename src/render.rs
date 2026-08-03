#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9PrimitiveType {
    TriangleStrip = 5,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct SrdRenderVertex {
    pub position: [f32; 3],
    pub primary_color: [u8; 4],
    pub secondary_color: [u8; 4],
    pub texture_coordinates: [[f32; 2]; 2],
}

impl SrdRenderVertex {
    pub const BINARY_FORMAT_ID: u32 = 14;
    pub const STRIDE: usize = 36;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrdQuadDraw {
    pub primitive_type: D3d9PrimitiveType,
    pub vertices: [SrdRenderVertex; 4],
}

impl SrdQuadDraw {
    pub const PRIMITIVE_COUNT: u32 = 2;

    pub fn new(vertices: [SrdRenderVertex; 4]) -> Self {
        Self {
            primitive_type: D3d9PrimitiveType::TriangleStrip,
            vertices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_layout_matches_the_binary_format_14_writes() {
        assert_eq!(std::mem::size_of::<SrdRenderVertex>(), 36);
        assert_eq!(std::mem::offset_of!(SrdRenderVertex, position), 0);
        assert_eq!(std::mem::offset_of!(SrdRenderVertex, primary_color), 12);
        assert_eq!(std::mem::offset_of!(SrdRenderVertex, secondary_color), 16);
        assert_eq!(
            std::mem::offset_of!(SrdRenderVertex, texture_coordinates),
            20
        );
        assert_eq!(D3d9PrimitiveType::TriangleStrip as u32, 5);
        assert_eq!(SrdQuadDraw::PRIMITIVE_COUNT, 2);
    }
}
