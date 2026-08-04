use std::fs;
use std::path::{Path, PathBuf};

use crate::scene::Project;
use crate::texture::TextureList;
use crate::vtbf::SrdFile;

#[derive(Debug, Clone)]
pub struct EditorDocument {
    pub path: PathBuf,
    pub file: SrdFile,
    pub project: Project,
    pub textures: TextureList,
}

impl EditorDocument {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let file = SrdFile::parse(bytes)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
        let project = Project::from_file(&file)
            .map_err(|error| format!("failed to decode {} scene data: {error}", path.display()))?;
        let textures = TextureList::from_file(&file)
            .map_err(|error| format!("failed to decode {} texture list: {error}", path.display()))?
            .unwrap_or(TextureList {
                declared_count: 0,
                textures: Vec::new(),
            });
        Ok(Self {
            path,
            file,
            project,
            textures,
        })
    }
}

pub fn display_srd_name(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
