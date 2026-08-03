use std::fmt;

use crate::vtbf::{Block, Property, SrdFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsliError(pub String);

impl fmt::Display for CsliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CsliError {}

#[derive(Debug, Clone, PartialEq)]
pub struct SlicCell {
    pub flags: u32,
    pub explicit_width: f32,
    pub explicit_height: f32,
    pub field_3a: Option<[u8; 4]>,
    pub field_33: Option<[u8; 4]>,
    pub field_44: Vec<[u8; 4]>,
    pub field_46: i16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CsliDefinition {
    pub field_80: u32,
    pub width: f32,
    pub height: f32,
    pub custom_origin: [f32; 2],
    pub field_44: [[u8; 4]; 4],
    pub origin_mode: u8,
    pub columns: u16,
    pub rows: u16,
    pub explicit_width_cell_count: u16,
    pub explicit_height_cell_count: u16,
    pub cref_count: u16,
    pub node_index: i32,
    pub cells: Vec<SlicCell>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneratedCellRect {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl CsliDefinition {
    pub fn from_block(file: &SrdFile, block: &Block) -> Result<Self, CsliError> {
        if !block.is_tag(b"CSLI") {
            return Err(CsliError("block is not CSLI".into()));
        }

        let mut result = Self {
            field_80: 0,
            width: 0.0,
            height: 0.0,
            custom_origin: [0.0, 0.0],
            field_44: [[0xff; 4]; 4],
            origin_mode: 0,
            columns: 0,
            rows: 0,
            explicit_width_cell_count: 0,
            explicit_height_cell_count: 0,
            cref_count: 0,
            node_index: -1,
            cells: Vec::new(),
        };
        let mut field_44_index = 0usize;
        for property in &block.properties {
            match property.code {
                0x80 => result.field_80 = unsigned_scalar(file, property, "CSLI 0x80")?,
                0x40 => result.width = float_scalar(file, property, "CSLI 0x40")?,
                0x41 => result.height = float_scalar(file, property, "CSLI 0x41")?,
                0x42 => result.custom_origin[0] = float_scalar(file, property, "CSLI 0x42")?,
                0x43 => result.custom_origin[1] = float_scalar(file, property, "CSLI 0x43")?,
                0x44 => {
                    let destination = result.field_44.get_mut(field_44_index).ok_or_else(|| {
                        CsliError("CSLI has more than four 0x44 properties".into())
                    })?;
                    *destination = reordered_four_bytes(file, property, "CSLI 0x44")?;
                    field_44_index += 1;
                }
                0x4b => result.origin_mode = unsigned_scalar(file, property, "CSLI 0x4b")? as u8,
                0x81 => result.columns = unsigned_scalar(file, property, "CSLI 0x81")? as u16,
                0x82 => result.rows = unsigned_scalar(file, property, "CSLI 0x82")? as u16,
                0x84 => {
                    result.explicit_width_cell_count =
                        unsigned_scalar(file, property, "CSLI 0x84")? as u16
                }
                0x85 => {
                    result.explicit_height_cell_count =
                        unsigned_scalar(file, property, "CSLI 0x85")? as u16
                }
                0x45 => result.cref_count = unsigned_scalar(file, property, "CSLI 0x45")? as u16,
                0x51 => result.node_index = unsigned_scalar(file, property, "CSLI 0x51")? as i32,
                _ => {}
            }
        }

        let slic_blocks = block
            .children
            .iter()
            .filter(|child| child.is_tag(b"SLIC"))
            .collect::<Vec<_>>();
        if slic_blocks.len() > 1 {
            return Err(CsliError("CSLI has more than one SLIC child".into()));
        }
        if let Some(slic) = slic_blocks.first() {
            result.cells = parse_slic(file, slic)?;
        }

        Ok(result)
    }

    pub fn expected_cell_count(&self) -> usize {
        usize::from(self.columns) * usize::from(self.rows)
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

    #[allow(clippy::assign_op_pattern)]
    pub fn generate_cell_rects(&self) -> Result<Vec<GeneratedCellRect>, CsliError> {
        let expected = self.expected_cell_count();
        if self.cells.len() != expected {
            return Err(CsliError(format!(
                "CSLI grid is {}x{} but SLIC contains {} records",
                self.columns,
                self.rows,
                self.cells.len()
            )));
        }
        if self.columns == 0 || self.rows == 0 {
            return Ok(Vec::new());
        }

        let mut first_row_explicit_width = 0.0f32;
        let mut first_column_explicit_height = 0.0f32;
        for row in 0..usize::from(self.rows) {
            for column in 0..usize::from(self.columns) {
                let cell = &self.cells[row * usize::from(self.columns) + column];
                if column == 0 && cell.flags & 0x02 != 0 {
                    // The game loads the new value first, then adds the accumulator.
                    first_column_explicit_height =
                        cell.explicit_height + first_column_explicit_height;
                }
                if row == 0 && cell.flags & 0x01 != 0 {
                    // Preserve the same operand order for signed zero and NaN payloads.
                    first_row_explicit_width = cell.explicit_width + first_row_explicit_width;
                }
            }
        }

        let denominator_x =
            ((i32::from(self.columns) - i32::from(self.explicit_width_cell_count)) as f32).max(1.0);
        let denominator_y =
            ((i32::from(self.rows) - i32::from(self.explicit_height_cell_count)) as f32).max(1.0);
        let default_width = (self.width - first_row_explicit_width) / denominator_x;
        let default_height = (self.height - first_column_explicit_height) / denominator_y;

        let mut generated = Vec::with_capacity(expected);
        let mut y = 0.0f32;
        for row in 0..usize::from(self.rows) {
            let row_start = row * usize::from(self.columns);
            let first_cell = &self.cells[row_start];
            let row_height = if first_cell.flags & 0x02 != 0 {
                first_cell.explicit_height
            } else {
                default_height
            };
            let mut x = 0.0f32;
            for column in 0..usize::from(self.columns) {
                let source = &self.cells[row_start + column];
                let width = if source.flags & 0x01 != 0 {
                    source.explicit_width
                } else {
                    default_width
                };
                generated.push(GeneratedCellRect {
                    active: (source.flags >> 8) & 1 != 0,
                    x,
                    y,
                    width,
                    height: row_height,
                });
                x += width;
            }
            y += row_height;
        }
        Ok(generated)
    }
}

pub fn parent_cell_center_offset(
    cell_index: i32,
    cells: &[GeneratedCellRect],
    parent_size: [f32; 2],
    axis_mode: bool,
) -> [f32; 2] {
    let Ok(index) = usize::try_from(cell_index) else {
        return [0.0, 0.0];
    };
    let Some(cell) = cells.get(index) else {
        return [0.0, 0.0];
    };
    if !cell.active {
        return [0.0, 0.0];
    }

    let left = cell.x - parent_size[0];
    let right = cell.width + left;
    let x = (right + left) * 0.5;

    let mut first = cell.y - parent_size[1];
    let mut second = (cell.height - parent_size[1]) + cell.y;
    if !axis_mode {
        second = -second;
        first = cell.height + second;
    }
    let y = (first + second) * 0.5;
    [x, y]
}

fn parse_slic(file: &SrdFile, block: &Block) -> Result<Vec<SlicCell>, CsliError> {
    let records = split_records(&block.properties);
    records
        .iter()
        .map(|properties| {
            let mut cell = SlicCell {
                flags: 0,
                explicit_width: 0.0,
                explicit_height: 0.0,
                field_3a: None,
                field_33: None,
                field_44: Vec::new(),
                field_46: 0,
            };
            for property in properties {
                match property.code {
                    0x83 => cell.flags = unsigned_scalar(file, property, "SLIC 0x83")?,
                    0x40 => cell.explicit_width = float_scalar(file, property, "SLIC 0x40")?,
                    0x41 => cell.explicit_height = float_scalar(file, property, "SLIC 0x41")?,
                    0x3a => {
                        cell.field_3a = Some(reordered_four_bytes(file, property, "SLIC 0x3a")?)
                    }
                    0x33 => {
                        cell.field_33 = Some(reordered_four_bytes(file, property, "SLIC 0x33")?)
                    }
                    0x44 => cell
                        .field_44
                        .push(reordered_four_bytes(file, property, "SLIC 0x44")?),
                    0x46 => cell.field_46 = signed_scalar(file, property, "SLIC 0x46")? as i16,
                    _ => {}
                }
            }
            if cell.flags & 0x200 == 0 {
                cell.flags |= 0x100;
            }
            Ok(cell)
        })
        .collect()
}

fn split_records(properties: &[Property]) -> Vec<Vec<&Property>> {
    let mut records = vec![Vec::new()];
    for property in properties {
        if property.code == 0xfe {
            records.push(Vec::new());
        } else {
            records.last_mut().unwrap().push(property);
        }
    }
    if records.last().is_some_and(Vec::is_empty) {
        records.pop();
    }
    records
}

fn unsigned_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<u32, CsliError> {
    property
        .read_unsigned_scalar(file)
        .ok_or_else(|| CsliError(format!("invalid {label}")))
}

fn signed_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<i32, CsliError> {
    property
        .read_signed_scalar(file)
        .ok_or_else(|| CsliError(format!("invalid {label}")))
}

fn float_scalar(file: &SrdFile, property: &Property, label: &str) -> Result<f32, CsliError> {
    property
        .read_scalar_as_f32(file)
        .ok_or_else(|| CsliError(format!("invalid {label}")))
}

fn reordered_four_bytes(
    file: &SrdFile,
    property: &Property,
    label: &str,
) -> Result<[u8; 4], CsliError> {
    let bytes = property.value_bytes(file);
    if bytes.len() < 4 {
        return Err(CsliError(format!("{label} has fewer than four bytes")));
    }
    Ok([bytes[1], bytes[2], bytes[3], bytes[0]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(flags: u32, width: f32, height: f32) -> SlicCell {
        SlicCell {
            flags,
            explicit_width: width,
            explicit_height: height,
            field_3a: None,
            field_33: None,
            field_44: Vec::new(),
            field_46: 0,
        }
    }

    #[test]
    fn cell_generation_uses_first_cell_height_for_each_row() {
        let definition = CsliDefinition {
            field_80: 0,
            width: 11.0,
            height: 23.0,
            custom_origin: [0.0, 0.0],
            field_44: [[0xff; 4]; 4],
            origin_mode: 0,
            columns: 2,
            rows: 2,
            explicit_width_cell_count: 0,
            explicit_height_cell_count: 0,
            cref_count: 0,
            node_index: 0,
            cells: vec![
                cell(0x102, 99.0, 7.0),
                cell(0x103, 3.0, 88.0),
                cell(0x100, 77.0, 66.0),
                cell(0x101, 4.0, 55.0),
            ],
        };
        let result = definition.generate_cell_rects().unwrap();
        assert_eq!(
            result,
            vec![
                GeneratedCellRect {
                    active: true,
                    x: 0.0,
                    y: 0.0,
                    width: 4.0,
                    height: 7.0,
                },
                GeneratedCellRect {
                    active: true,
                    x: 4.0,
                    y: 0.0,
                    width: 3.0,
                    height: 7.0,
                },
                GeneratedCellRect {
                    active: true,
                    x: 0.0,
                    y: 7.0,
                    width: 4.0,
                    height: 8.0,
                },
                GeneratedCellRect {
                    active: true,
                    x: 4.0,
                    y: 7.0,
                    width: 4.0,
                    height: 8.0,
                },
            ]
        );
    }

    #[test]
    fn center_offset_matches_both_binary_branches() {
        let cells = [GeneratedCellRect {
            active: true,
            x: 13.0,
            y: 17.0,
            width: 6.0,
            height: 8.0,
        }];
        assert_eq!(
            parent_cell_center_offset(0, &cells, [10.0, 12.0], true),
            [6.0, 9.0]
        );
        assert_eq!(
            parent_cell_center_offset(0, &cells, [10.0, 12.0], false),
            [6.0, -9.0]
        );
        assert_eq!(
            parent_cell_center_offset(-1, &cells, [10.0, 12.0], true),
            [0.0, 0.0]
        );
        assert_eq!(
            parent_cell_center_offset(1, &cells, [10.0, 12.0], true),
            [0.0, 0.0]
        );
        let inactive = [GeneratedCellRect {
            active: false,
            ..cells[0]
        }];
        assert_eq!(
            parent_cell_center_offset(0, &inactive, [10.0, 12.0], true),
            [0.0, 0.0]
        );
    }

    #[test]
    fn runtime_origin_uses_the_binary_nine_mode_table_and_custom_fallback() {
        let mut definition = CsliDefinition {
            field_80: 0,
            width: 20.0,
            height: 30.0,
            custom_origin: [7.0, 9.0],
            field_44: [[0xff; 4]; 4],
            origin_mode: 4,
            columns: 0,
            rows: 0,
            explicit_width_cell_count: 0,
            explicit_height_cell_count: 0,
            cref_count: 0,
            node_index: 0,
            cells: Vec::new(),
        };
        assert_eq!(definition.runtime_origin_offset(), [10.0, 15.0]);
        definition.origin_mode = 8;
        assert_eq!(definition.runtime_origin_offset(), [20.0, 30.0]);
        definition.origin_mode = 9;
        assert_eq!(definition.runtime_origin_offset(), [7.0, 9.0]);
    }
}
