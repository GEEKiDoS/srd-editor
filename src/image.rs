use std::fmt;

use crate::csli::{CrefEntry, slice_texture_coordinates};
use crate::texture::TextureList;
use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError(pub String);

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ImageError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageReferenceChannel {
    Cref,
    Cre1,
}

impl ImageReferenceChannel {
    fn index(self) -> usize {
        match self {
            Self::Cref => 0,
            Self::Cre1 => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageCoordinateState {
    pub vertex_colors: [[u8; 4]; 4],
    pub reference_index: i16,
    pub explicit_image_index: i16,
    pub uses_explicit_rectangle: bool,
    pub explicit_rectangle: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedImageCoordinates {
    pub image_index: i16,
    pub coordinates: [[f32; 2]; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageQuad {
    pub positions: [[f32; 3]; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageDefinition {
    pub flags: u32,
    pub width: f32,
    pub height: f32,
    pub custom_origin: [f32; 2],
    pub origin_mode: u8,
    pub vertex_colors: [[u8; 4]; 4],
    pub cref_index: i16,
    pub cref_count: u16,
    pub crefs: Vec<CrefEntry>,
    pub field_4c: u16,
    pub cre1_index: i16,
    pub cre1_count: u16,
    pub cre1s: Vec<CrefEntry>,
    pub coordinate_offsets: [[f32; 2]; 2],
    pub field_a1: u32,
    pub node_index: i32,
    pub has_text_child: bool,
}

impl ImageDefinition {
    pub const INITIAL_COORDINATE_OFFSET_SCALE: f32 = f32::from_bits(0x4cbe_bc20);

    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, ImageError> {
        if !block.is_tag(b"CIMG") {
            return Err(ImageError("block is not CIMG".into()));
        }

        let mut result = Self {
            flags: 0,
            width: 128.0,
            height: 128.0,
            custom_origin: [0.0, 0.0],
            origin_mode: 0,
            vertex_colors: [[0xff; 4]; 4],
            cref_index: -1,
            cref_count: 0,
            crefs: Vec::new(),
            field_4c: 0,
            cre1_index: -1,
            cre1_count: 0,
            cre1s: Vec::new(),
            coordinate_offsets: [[0.0; 2]; 2],
            field_a1: 0,
            node_index: -1,
            has_text_child: false,
        };
        let mut color_index = 0usize;
        for property in &block.properties {
            match property.code {
                0x40 => result.width = float_scalar(file, property, "CIMG 0x40")?,
                0x41 => result.height = float_scalar(file, property, "CIMG 0x41")?,
                0x42 => result.custom_origin[0] = float_scalar(file, property, "CIMG 0x42")?,
                0x43 => result.custom_origin[1] = float_scalar(file, property, "CIMG 0x43")?,
                0x44 => {
                    let destination =
                        result.vertex_colors.get_mut(color_index).ok_or_else(|| {
                            ImageError("CIMG has more than four 0x44 properties".into())
                        })?;
                    *destination = reordered_four_bytes(file, property, "CIMG 0x44")?;
                    color_index += 1;
                }
                0x45 => result.cref_count = unsigned_scalar(file, property, "CIMG 0x45")? as u16,
                0x46 => result.cref_index = signed_scalar(file, property, "CIMG 0x46")? as i16,
                0x49 => result.flags = unsigned_scalar(file, property, "CIMG 0x49")?,
                0x4b => result.origin_mode = unsigned_scalar(file, property, "CIMG 0x4b")? as u8,
                0x4c => result.field_4c = unsigned_scalar(file, property, "CIMG 0x4c")? as u16,
                0x4d => result.cre1_count = unsigned_scalar(file, property, "CIMG 0x4d")? as u16,
                0x4e => result.cre1_index = signed_scalar(file, property, "CIMG 0x4e")? as i16,
                0x51 => result.node_index = unsigned_scalar(file, property, "CIMG 0x51")? as i32,
                0x83 => {
                    result.coordinate_offsets[0][0] = float_scalar(file, property, "CIMG 0x83")?
                }
                0x84 => {
                    result.coordinate_offsets[1][0] = float_scalar(file, property, "CIMG 0x84")?
                }
                0x85 => {
                    result.coordinate_offsets[0][1] = float_scalar(file, property, "CIMG 0x85")?
                }
                0x86 => {
                    result.coordinate_offsets[1][1] = float_scalar(file, property, "CIMG 0x86")?
                }
                0xa1 => result.field_a1 = unsigned_scalar(file, property, "CIMG 0xa1")?,
                _ => {}
            }
        }

        for child in &block.children {
            if child.is_tag(b"CREF") && result.cref_count != 0 {
                result.crefs = parse_reference_table(file, child, result.cref_count, "CREF")?;
            } else if child.is_tag(b"CRE1") && result.cre1_count != 0 {
                result.cre1s = parse_reference_table(file, child, result.cre1_count, "CRE1")?;
            } else if child.is_tag(b"TEXT") {
                result.has_text_child = true;
            }
        }

        Ok(result)
    }

    pub fn runtime_origin_offset(&self) -> [f32; 2] {
        const FACTORS: [[f32; 2]; 9] = [
            [0.0, 0.0],
            [0.5, 0.0],
            [1.0, 0.0],
            [0.0, 0.5],
            [0.5, 0.5],
            [1.0, 0.5],
            [0.0, 1.0],
            [0.5, 1.0],
            [1.0, 1.0],
        ];
        let Some(factors) = FACTORS.get(usize::from(self.origin_mode)) else {
            return self.custom_origin;
        };
        [factors[0] * self.width, factors[1] * self.height]
    }

    pub fn point_sampled(&self) -> bool {
        self.flags & 0x0100_0000 != 0
    }

    pub fn creates_text_cast(&self) -> bool {
        self.has_text_child && self.flags & 0x100 != 0
    }

    pub fn initial_coordinate_state(&self, channel: ImageReferenceChannel) -> ImageCoordinateState {
        ImageCoordinateState {
            vertex_colors: self.vertex_colors,
            reference_index: match channel {
                ImageReferenceChannel::Cref => self.cref_index,
                ImageReferenceChannel::Cre1 => self.cre1_index,
            },
            explicit_image_index: 0,
            uses_explicit_rectangle: false,
            explicit_rectangle: [0.0; 4],
        }
    }

    pub fn build_quad(&self, axis_mode: bool) -> ImageQuad {
        let origin = self.runtime_origin_offset();
        let left = -origin[0];
        let right = self.width - origin[0];
        let mut first_y = -origin[1];
        let mut second_y = self.height - origin[1];
        if !axis_mode {
            first_y = -first_y;
            second_y = -second_y;
        }
        ImageQuad {
            positions: [
                [left, first_y, 0.0],
                [left, second_y, 0.0],
                [right, first_y, 0.0],
                [right, second_y, 0.0],
            ],
        }
    }

    #[allow(clippy::assign_op_pattern)]
    pub fn resolve_coordinates(
        &self,
        channel: ImageReferenceChannel,
        state: ImageCoordinateState,
        textures: &TextureList,
        offset_scale: f32,
    ) -> Result<ResolvedImageCoordinates, ImageError> {
        let channel_index = channel.index();
        let (declared_count, references) = match channel {
            ImageReferenceChannel::Cref => (self.cref_count, self.crefs.as_slice()),
            ImageReferenceChannel::Cre1 => (self.cre1_count, self.cre1s.as_slice()),
        };
        let selector = state.reference_index;
        let mut image_index = -1i16;
        let mut rectangle = [0.0f32; 4];

        if selector >= 0 && u32::from(selector as u16) < u32::from(declared_count) {
            if state.uses_explicit_rectangle {
                image_index = state.explicit_image_index;
                rectangle = state.explicit_rectangle;
            } else if !references.is_empty() {
                let reference = references.get(selector as usize).ok_or_else(|| {
                    ImageError(format!(
                        "{channel:?} selector {selector} is below declared count {declared_count} but outside its allocated table"
                    ))
                })?;
                image_index = reference.image_index;
                if reference.image_index >= 0 && reference.rectangle_index >= 0 {
                    let texture = textures
                        .textures
                        .get(reference.image_index as usize)
                        .ok_or_else(|| {
                            ImageError(format!(
                                "{channel:?} image index {} is outside {} TEX records",
                                reference.image_index,
                                textures.textures.len()
                            ))
                        })?;
                    rectangle = texture
                        .crops
                        .get(reference.rectangle_index as usize)
                        .ok_or_else(|| {
                            ImageError(format!(
                                "{channel:?} rectangle index {} is outside {} CROP records for TEX {}",
                                reference.rectangle_index,
                                texture.crops.len(),
                                reference.image_index
                            ))
                        })?
                        .normalized_rectangle;
                }
            }
        }

        let mut coordinates = slice_texture_coordinates(rectangle, self.flags);
        let offset = self.coordinate_offsets[channel_index];
        for coordinate in &mut coordinates {
            coordinate[0] = offset[0] * offset_scale + coordinate[0];
            coordinate[1] = offset[1] * offset_scale + coordinate[1];
        }
        Ok(ResolvedImageCoordinates {
            image_index,
            coordinates,
        })
    }
}

fn parse_reference_table(
    file: &SrdFile,
    block: &Block,
    declared_count: u16,
    label: &str,
) -> Result<Vec<CrefEntry>, ImageError> {
    let mut entries = vec![
        CrefEntry {
            image_index: 0,
            rectangle_index: 0,
        };
        usize::from(declared_count)
    ];
    for (index, property) in block.properties_with_code(0x4a).enumerate() {
        let destination = entries.get_mut(index).ok_or_else(|| {
            ImageError(format!(
                "CIMG declares {declared_count} {label} records but the child has more"
            ))
        })?;
        destination.image_index = property
            .read_signed_scalar_at(file, 0)
            .ok_or_else(|| ImageError(format!("invalid {label} 0x4a image index")))?
            as i16;
        destination.rectangle_index = property
            .read_signed_scalar_at(file, 1)
            .ok_or_else(|| ImageError(format!("invalid {label} 0x4a rectangle index")))?
            as i16;
    }
    Ok(entries)
}

fn unsigned_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<u32, ImageError> {
    property
        .read_unsigned_scalar(file)
        .ok_or_else(|| ImageError(format!("invalid {label}")))
}

fn signed_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<i32, ImageError> {
    property
        .read_signed_scalar(file)
        .ok_or_else(|| ImageError(format!("invalid {label}")))
}

fn float_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<f32, ImageError> {
    property
        .read_scalar_as_f32(file)
        .ok_or_else(|| ImageError(format!("invalid {label}")))
}

fn reordered_four_bytes(
    file: &SrdFile,
    property: &Property,
    label: &str,
) -> Result<[u8; 4], ImageError> {
    let bytes = property.value_bytes(file);
    if bytes.len() < 4 {
        return Err(ImageError(format!("{label} has fewer than four bytes")));
    }
    Ok([bytes[1], bytes[2], bytes[3], bytes[0]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::{TextureCrop, TextureDefinition};

    fn definition() -> ImageDefinition {
        ImageDefinition {
            flags: 0,
            width: 8.0,
            height: 6.0,
            custom_origin: [1.0, 2.0],
            origin_mode: 9,
            vertex_colors: [[0xff; 4]; 4],
            cref_index: 0,
            cref_count: 1,
            crefs: vec![CrefEntry {
                image_index: 0,
                rectangle_index: 0,
            }],
            field_4c: 0,
            cre1_index: -1,
            cre1_count: 0,
            cre1s: Vec::new(),
            coordinate_offsets: [[0.0; 2]; 2],
            field_a1: 0,
            node_index: 0,
            has_text_child: false,
        }
    }

    fn textures() -> TextureList {
        TextureList {
            declared_count: 1,
            textures: vec![TextureDefinition {
                crops: vec![TextureCrop {
                    normalized_rectangle: [0.1, 0.2, 0.7, 0.9],
                }],
                crop_count: 1,
                ..TextureDefinition::default()
            }],
        }
    }

    #[test]
    fn cref_and_cre1_are_independent_channels() {
        let mut definition = definition();
        definition.cre1_index = 0;
        definition.cre1_count = 1;
        definition.cre1s = vec![CrefEntry {
            image_index: 0,
            rectangle_index: 0,
        }];
        definition.coordinate_offsets = [[0.25, 0.5], [1.0, 2.0]];

        let first = definition
            .resolve_coordinates(
                ImageReferenceChannel::Cref,
                definition.initial_coordinate_state(ImageReferenceChannel::Cref),
                &textures(),
                2.0,
            )
            .unwrap();
        let second = definition
            .resolve_coordinates(
                ImageReferenceChannel::Cre1,
                definition.initial_coordinate_state(ImageReferenceChannel::Cre1),
                &textures(),
                2.0,
            )
            .unwrap();
        assert_eq!(first.coordinates[0], [0.6, 1.2]);
        assert_eq!(second.coordinates[0], [2.1, 4.2]);
    }

    #[test]
    fn explicit_rectangle_still_requires_selector_inside_declared_count() {
        let definition = definition();
        let state = ImageCoordinateState {
            reference_index: -1,
            explicit_image_index: 7,
            uses_explicit_rectangle: true,
            explicit_rectangle: [0.2, 0.3, 0.4, 0.5],
            ..definition.initial_coordinate_state(ImageReferenceChannel::Cref)
        };
        let resolved = definition
            .resolve_coordinates(ImageReferenceChannel::Cref, state, &textures(), 1.0)
            .unwrap();
        assert_eq!(resolved.image_index, -1);
        assert_eq!(resolved.coordinates, [[0.0; 2]; 4]);
    }

    #[test]
    fn image_quad_preserves_both_axis_orders() {
        let definition = definition();
        assert_eq!(
            definition.build_quad(true).positions,
            [
                [-1.0, -2.0, 0.0],
                [-1.0, 4.0, 0.0],
                [7.0, -2.0, 0.0],
                [7.0, 4.0, 0.0],
            ]
        );
        assert_eq!(
            definition.build_quad(false).positions,
            [
                [-1.0, 2.0, 0.0],
                [-1.0, -4.0, 0.0],
                [7.0, 2.0, 0.0],
                [7.0, -4.0, 0.0],
            ]
        );
    }
}
