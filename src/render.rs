#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum D3d9PrimitiveType {
    TriangleStrip = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum D3d9DeclarationType {
    Float2 = 1,
    Float3 = 2,
    Color = 4,
    Unused = 17,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum D3d9DeclarationUsage {
    Position = 0,
    TextureCoordinate = 5,
    Color = 10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct D3d9VertexElement {
    pub stream: u16,
    pub offset: u16,
    pub declaration_type: u8,
    pub method: u8,
    pub usage: u8,
    pub usage_index: u8,
}

pub const SRD_D3D9_VERTEX_DECLARATION: [D3d9VertexElement; 6] = [
    D3d9VertexElement {
        stream: 0,
        offset: 0,
        declaration_type: D3d9DeclarationType::Float3 as u8,
        method: 0,
        usage: D3d9DeclarationUsage::Position as u8,
        usage_index: 0,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 12,
        declaration_type: D3d9DeclarationType::Color as u8,
        method: 0,
        usage: D3d9DeclarationUsage::Color as u8,
        usage_index: 0,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 16,
        declaration_type: D3d9DeclarationType::Color as u8,
        method: 0,
        usage: D3d9DeclarationUsage::Color as u8,
        usage_index: 1,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 20,
        declaration_type: D3d9DeclarationType::Float2 as u8,
        method: 0,
        usage: D3d9DeclarationUsage::TextureCoordinate as u8,
        usage_index: 0,
    },
    D3d9VertexElement {
        stream: 0,
        offset: 28,
        declaration_type: D3d9DeclarationType::Float2 as u8,
        method: 0,
        usage: D3d9DeclarationUsage::TextureCoordinate as u8,
        usage_index: 1,
    },
    D3d9VertexElement {
        stream: 0xff,
        offset: 0,
        declaration_type: D3d9DeclarationType::Unused as u8,
        method: 0,
        usage: 0,
        usage_index: 0,
    },
];

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
        assert_eq!(std::mem::size_of::<D3d9VertexElement>(), 8);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[0].offset, 0);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[1].offset, 12);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[2].offset, 16);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[3].offset, 20);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[4].offset, 28);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[5].stream, 0xff);
        assert_eq!(SRD_D3D9_VERTEX_DECLARATION[5].declaration_type, 17);
    }
}
