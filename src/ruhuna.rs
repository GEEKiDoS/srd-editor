use std::fmt;
use std::ops::Range;

use crate::avts::{AvtsError, AvtsFile};
use crate::dds::{DdsDescriptor, DdsError};
use crate::yabx::{RfzYabxError, YabxFile, YabxObject};

const DATABASE_CLASS: &[u8] = b"ruhuna::Database";
const GLYPH_CLASS: &[u8] = b"ruhuna::Glyph";
const TEXTURE_RESOURCE_CLASS: &[u8] = b"ruhuna::TextureResource";
const SERIALIZED_OBJECT_ID_BASE: i32 = 10_001;

#[derive(Debug)]
pub enum RuhunaError {
    Container(RfzYabxError),
    Avts(AvtsError),
    Dds(DdsError),
    Invalid(String),
}

impl fmt::Display for RuhunaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => error.fmt(formatter),
            Self::Avts(error) => error.fmt(formatter),
            Self::Dds(error) => error.fmt(formatter),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RuhunaError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuhunaDatabase {
    pub object_index: usize,
    pub id: Vec<u8>,
    pub platform: Vec<u8>,
    pub library: Vec<u8>,
    pub name: Vec<u8>,
    pub comment: Vec<u8>,
    pub flags: u32,
    pub point: u16,
    pub max_ascent: u16,
    pub max_descent: u16,
    pub max_glyph_width: u16,
    pub max_glyph_height: u16,
    pub texture_page_count: u32,
    pub texture_width: u16,
    pub texture_height: u16,
    pub texture_last_height: u16,
    pub glyph_margin: u16,
    pub glyph_count: u16,
    pub glyph_object_indices: Vec<usize>,
    pub texture_object_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuhunaGlyph {
    pub object_index: usize,
    pub code: u16,
    pub cell_increment_x: u16,
    pub cell_increment_y: u16,
    pub page: u16,
    pub origin_x: i16,
    pub origin_y: i16,
    pub box_x1: u16,
    pub box_y1: u16,
    pub box_x2: u16,
    pub box_y2: u16,
    pub kerning_info_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuhunaTextureResource {
    pub object_index: usize,
    file_data: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuhunaAtlasPage {
    pub page_index: usize,
    pub entry_name: Vec<u8>,
    pub descriptor: DdsDescriptor,
    data: Range<usize>,
}

#[derive(Debug)]
pub struct RuhunaFont {
    yabx: YabxFile,
    pub database: RuhunaDatabase,
    pub glyphs: Vec<RuhunaGlyph>,
    pub textures: Vec<RuhunaTextureResource>,
}

impl RuhunaFont {
    pub fn from_rfz(bytes: &[u8]) -> Result<Self, RuhunaError> {
        let yabx = YabxFile::from_rfz(bytes).map_err(RuhunaError::Container)?;
        Self::from_yabx(yabx)
    }

    pub fn from_yabx(yabx: YabxFile) -> Result<Self, RuhunaError> {
        let database_objects = objects_of_class(&yabx, DATABASE_CLASS);
        if database_objects.len() != 1 {
            return Err(RuhunaError::Invalid(format!(
                "ruhuna font needs exactly one Database object, found {}",
                database_objects.len()
            )));
        }
        let (database_index, database_object) = database_objects[0];
        let database = parse_database(&yabx, database_index, database_object)?;

        let glyphs = objects_of_class(&yabx, GLYPH_CLASS)
            .into_iter()
            .map(|(index, object)| parse_glyph(&yabx, index, object))
            .collect::<Result<Vec<_>, _>>()?;
        let textures = objects_of_class(&yabx, TEXTURE_RESOURCE_CLASS)
            .into_iter()
            .map(|(index, object)| parse_texture_resource(&yabx, index, object))
            .collect::<Result<Vec<_>, _>>()?;

        if usize::from(database.glyph_count) != database.glyph_object_indices.len() {
            return Err(RuhunaError::Invalid(format!(
                "Database glyph_count {} does not match {} serialized glyph references",
                database.glyph_count,
                database.glyph_object_indices.len()
            )));
        }
        validate_referenced_classes(&yabx, &database.glyph_object_indices, GLYPH_CLASS, "glyph")?;
        validate_referenced_classes(
            &yabx,
            &database.texture_object_indices,
            TEXTURE_RESOURCE_CLASS,
            "texture",
        )?;

        Ok(Self {
            yabx,
            database,
            glyphs,
            textures,
        })
    }

    pub fn yabx(&self) -> &YabxFile {
        &self.yabx
    }

    pub fn texture_file_bytes(&self, texture: &RuhunaTextureResource) -> &[u8] {
        &self.yabx.bytes()[texture.file_data.clone()]
    }

    pub fn texture_avts<'a>(
        &'a self,
        texture: &RuhunaTextureResource,
    ) -> Result<AvtsFile<'a>, AvtsError> {
        AvtsFile::parse(self.texture_file_bytes(texture))
    }

    pub fn atlas_pages(
        &self,
        texture: &RuhunaTextureResource,
    ) -> Result<Vec<RuhunaAtlasPage>, RuhunaError> {
        let avts = self.texture_avts(texture).map_err(RuhunaError::Avts)?;
        let metadata = avts
            .entries
            .first()
            .ok_or_else(|| RuhunaError::Invalid("font AVTS has no metadata entry".into()))?;
        if metadata.field_200 != 0
            || metadata.field_204 != 0
            || !avts.entry_data(metadata).starts_with(b"YABX")
        {
            return Err(RuhunaError::Invalid(
                "font AVTS entry 0 is not the binary-proven YABX metadata entry".into(),
            ));
        }

        let mut pages = Vec::new();
        for (entry_index, entry) in avts.entries.iter().enumerate().skip(1) {
            let page_index = entry_index - 1;
            if entry.field_200 != 1 || entry.field_204 != entry_index as u32 {
                return Err(RuhunaError::Invalid(format!(
                    "font AVTS entry {entry_index} has unexpected fields {:#x}/{:#x}",
                    entry.field_200, entry.field_204
                )));
            }
            let data = avts.entry_data(entry);
            let descriptor = DdsDescriptor::parse(data).map_err(RuhunaError::Dds)?;
            descriptor
                .validate_data_len(data)
                .map_err(RuhunaError::Dds)?;
            if descriptor.width != u32::from(self.database.texture_width) {
                return Err(RuhunaError::Invalid(format!(
                    "font atlas page {page_index} width {} does not match Database tex_w {}",
                    descriptor.width, self.database.texture_width
                )));
            }
            let expected_height = if entry_index + 1 == avts.entries.len() {
                self.database.texture_last_height
            } else {
                self.database.texture_height
            };
            if descriptor.height != u32::from(expected_height) {
                return Err(RuhunaError::Invalid(format!(
                    "font atlas page {page_index} height {} does not match expected {expected_height}",
                    descriptor.height
                )));
            }
            pages.push(RuhunaAtlasPage {
                page_index,
                entry_name: entry.name.clone(),
                descriptor,
                data: (texture.file_data.start + entry.data.start)
                    ..(texture.file_data.start + entry.data.end),
            });
        }
        if pages.len() != self.database.texture_page_count as usize {
            return Err(RuhunaError::Invalid(format!(
                "font AVTS has {} DDS pages but Database tex_page is {}",
                pages.len(),
                self.database.texture_page_count
            )));
        }
        Ok(pages)
    }

    pub fn atlas_page_bytes(&self, page: &RuhunaAtlasPage) -> &[u8] {
        &self.yabx.bytes()[page.data.clone()]
    }
}

fn objects_of_class<'a>(yabx: &'a YabxFile, expected_name: &[u8]) -> Vec<(usize, &'a YabxObject)> {
    yabx.objects
        .iter()
        .enumerate()
        .filter(|(_, object)| {
            yabx.object_class(object)
                .is_some_and(|class| class.name == expected_name)
        })
        .collect()
}

fn parse_database(
    yabx: &YabxFile,
    object_index: usize,
    object: &YabxObject,
) -> Result<RuhunaDatabase, RuhunaError> {
    let mut reader = ObjectReader::new(yabx.object_data(object), object_index);
    let database = RuhunaDatabase {
        object_index,
        id: reader.read_dynamic_string("id")?,
        platform: reader.read_dynamic_string("platform")?,
        library: reader.read_dynamic_string("library")?,
        name: reader.read_dynamic_string("name")?,
        comment: reader.read_dynamic_string("comment")?,
        flags: reader.read_u32("flags")?,
        point: reader.read_u16("point")?,
        max_ascent: reader.read_u16("max_ascent")?,
        max_descent: reader.read_u16("max_descent")?,
        max_glyph_width: reader.read_u16("max_glyph_w")?,
        max_glyph_height: reader.read_u16("max_glyph_h")?,
        texture_page_count: reader.read_u32("tex_page")?,
        texture_width: reader.read_u16("tex_w")?,
        texture_height: reader.read_u16("tex_h")?,
        texture_last_height: reader.read_u16("tex_last_h")?,
        glyph_margin: reader.read_u16("glyph_margin")?,
        glyph_count: reader.read_u16("glyph_cnt")?,
        glyph_object_indices: reader.read_dynamic_object_references("glyph", yabx.objects.len())?,
        texture_object_indices: reader
            .read_dynamic_object_references("texture", yabx.objects.len())?,
    };
    reader.finish()?;
    Ok(database)
}

fn parse_glyph(
    yabx: &YabxFile,
    object_index: usize,
    object: &YabxObject,
) -> Result<RuhunaGlyph, RuhunaError> {
    let mut reader = ObjectReader::new(yabx.object_data(object), object_index);
    let glyph = RuhunaGlyph {
        object_index,
        code: reader.read_u16("code")?,
        cell_increment_x: reader.read_u16("cell_inc_x")?,
        cell_increment_y: reader.read_u16("cell_inc_y")?,
        page: reader.read_u16("page")?,
        origin_x: reader.read_i16("origin_x")?,
        origin_y: reader.read_i16("origin_y")?,
        box_x1: reader.read_u16("box_x1")?,
        box_y1: reader.read_u16("box_y1")?,
        box_x2: reader.read_u16("box_x2")?,
        box_y2: reader.read_u16("box_y2")?,
        kerning_info_count: reader.read_u16("kerning_info_cnt")?,
    };
    let mut kerning = reader.read_dynamic_reader("kerning_info")?;
    let serialized_count = kerning.read_u32("kerning_info count")?;
    if serialized_count != u32::from(glyph.kerning_info_count) {
        return Err(reader.error(format!(
            "kerning_info count {serialized_count} does not match kerning_info_cnt {}",
            glyph.kerning_info_count
        )));
    }
    if serialized_count != 0 {
        return Err(reader.error(format!(
            "non-empty kerning_info arrays are not implemented without a proven element layout (count {serialized_count})"
        )));
    }
    kerning.finish()?;
    reader.finish()?;
    Ok(glyph)
}

fn parse_texture_resource(
    yabx: &YabxFile,
    object_index: usize,
    object: &YabxObject,
) -> Result<RuhunaTextureResource, RuhunaError> {
    let mut reader = ObjectReader::new(yabx.object_data(object), object_index);
    let local_range = reader.read_dynamic_range("file")?;
    reader.finish()?;
    let file_data = (object.data.start + local_range.start)..(object.data.start + local_range.end);
    let bytes = &yabx.bytes()[file_data.clone()];
    if !bytes.starts_with(b"AVTS") {
        return Err(RuhunaError::Invalid(format!(
            "TextureResource object {object_index} file does not start with AVTS"
        )));
    }
    Ok(RuhunaTextureResource {
        object_index,
        file_data,
    })
}

fn validate_referenced_classes(
    yabx: &YabxFile,
    object_indices: &[usize],
    expected_class: &[u8],
    label: &str,
) -> Result<(), RuhunaError> {
    for &object_index in object_indices {
        let object = &yabx.objects[object_index];
        let actual = yabx
            .object_class(object)
            .map(|class| class.name.as_slice())
            .unwrap_or(b"<unknown>");
        if actual != expected_class {
            return Err(RuhunaError::Invalid(format!(
                "Database {label} reference points to object {object_index} of class {:?}",
                String::from_utf8_lossy(actual)
            )));
        }
    }
    Ok(())
}

struct ObjectReader<'a> {
    bytes: &'a [u8],
    offset: usize,
    object_index: usize,
}

impl<'a> ObjectReader<'a> {
    fn new(bytes: &'a [u8], object_index: usize) -> Self {
        Self {
            bytes,
            offset: 0,
            object_index,
        }
    }

    fn error(&self, message: impl Into<String>) -> RuhunaError {
        RuhunaError::Invalid(format!(
            "ruhuna object {} at {:#x}: {}",
            self.object_index,
            self.offset,
            message.into()
        ))
    }

    fn read_bytes(&mut self, size: usize, label: &str) -> Result<&'a [u8], RuhunaError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| self.error(format!("{label} size overflows")))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| self.error(format!("{label} needs {size} bytes")))?;
        self.offset = end;
        Ok(bytes)
    }

    fn read_u16(&mut self, label: &str) -> Result<u16, RuhunaError> {
        Ok(u16::from_le_bytes(
            self.read_bytes(2, label)?.try_into().unwrap(),
        ))
    }

    fn read_i16(&mut self, label: &str) -> Result<i16, RuhunaError> {
        Ok(i16::from_le_bytes(
            self.read_bytes(2, label)?.try_into().unwrap(),
        ))
    }

    fn read_u32(&mut self, label: &str) -> Result<u32, RuhunaError> {
        Ok(u32::from_le_bytes(
            self.read_bytes(4, label)?.try_into().unwrap(),
        ))
    }

    fn read_dynamic_range(&mut self, label: &str) -> Result<Range<usize>, RuhunaError> {
        let size = self.read_u32(&format!("{label} byte size"))?;
        let size = usize::try_from(size)
            .map_err(|_| self.error(format!("{label} byte size does not fit usize")))?;
        let start = self.offset;
        self.read_bytes(size, label)?;
        Ok(start..self.offset)
    }

    fn read_dynamic_reader(&mut self, label: &str) -> Result<Self, RuhunaError> {
        let range = self.read_dynamic_range(label)?;
        Ok(Self::new(&self.bytes[range], self.object_index))
    }

    fn read_dynamic_string(&mut self, label: &str) -> Result<Vec<u8>, RuhunaError> {
        let mut field = self.read_dynamic_reader(label)?;
        let length = usize::from(field.read_u16(&format!("{label} string length"))?);
        if length == 0 {
            return Err(field.error(format!("{label} string length is zero")));
        }
        let bytes = field.read_bytes(length, label)?;
        if bytes.last() != Some(&0) {
            return Err(field.error(format!("{label} string is not NUL-terminated")));
        }
        field.finish()?;
        Ok(bytes[..bytes.len() - 1].to_vec())
    }

    fn read_dynamic_object_references(
        &mut self,
        label: &str,
        object_count: usize,
    ) -> Result<Vec<usize>, RuhunaError> {
        let mut field = self.read_dynamic_reader(label)?;
        let count = field.read_u32(&format!("{label} reference count"))?;
        let count = usize::try_from(count)
            .map_err(|_| field.error(format!("{label} reference count does not fit usize")))?;
        let mut references = Vec::with_capacity(count);
        for index in 0..count {
            let encoded = field.read_i16(&format!("{label}[{index}]"))?;
            references.push(
                decode_object_reference(encoded, object_count).map_err(|message| {
                    field.error(format!("invalid {label}[{index}] reference: {message}"))
                })?,
            );
        }
        field.finish()?;
        Ok(references)
    }

    fn finish(&self) -> Result<(), RuhunaError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(self.error(format!(
                "{} unread bytes remain",
                self.bytes.len() - self.offset
            )))
        }
    }
}

fn decode_object_reference(encoded: i16, object_count: usize) -> Result<usize, String> {
    if encoded <= 0 {
        return Err(format!(
            "encoded ID {encoded} is null or an external/local-library reference"
        ));
    }
    let index = i32::from(encoded) - SERIALIZED_OBJECT_ID_BASE;
    let index = usize::try_from(index)
        .map_err(|_| format!("encoded ID {encoded} is below the object ID base"))?;
    if index >= object_count {
        return Err(format!(
            "encoded ID {encoded} resolves to object {index}, outside {object_count} objects"
        ));
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_game_object_id_base() {
        assert_eq!(decode_object_reference(10_001, 2).unwrap(), 0);
        assert_eq!(decode_object_reference(10_002, 2).unwrap(), 1);
        assert!(decode_object_reference(10_000, 2).is_err());
        assert!(decode_object_reference(-1, 2).is_err());
    }
}
