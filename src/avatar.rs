use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct ManifestFile {
    schema_version: u32,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    type_: String,
    hitbox: Hitbox,
    clips: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Hitbox {
    pub width: f64,
    // Read from the manifest for schema completeness (puck-mac clicks/throws
    // by it); this MVP slice only needs width, for walk-edge turnaround.
    #[allow(dead_code)]
    pub height: f64,
}

#[derive(Debug)]
pub struct Avatar {
    pub hitbox: Hitbox,
    pub clips: HashMap<String, PathBuf>,
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    UnsupportedSchemaVersion(u32),
    MissingIdleClip,
    PathEscapesPackage(String),
    MissingClipFile { clip: String, path: PathBuf },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "could not read avatar package: {e}"),
            LoadError::Parse(e) => write!(f, "manifest.json is not valid JSON: {e}"),
            LoadError::UnsupportedSchemaVersion(v) => {
                write!(f, "unsupported manifest schema_version: {v}")
            }
            LoadError::MissingIdleClip => {
                write!(f, "manifest.json must define an 'idle' clip")
            }
            LoadError::PathEscapesPackage(stem) => {
                write!(f, "clip path '{stem}' escapes the avatar package directory")
            }
            LoadError::MissingClipFile { clip, path } => {
                write!(f, "clip '{clip}' references missing file {}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

pub fn load(dir: &Path) -> Result<Avatar, LoadError> {
    let manifest_path = dir.join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path).map_err(LoadError::Io)?;
    let manifest: ManifestFile = serde_json::from_str(&raw).map_err(LoadError::Parse)?;

    if manifest.schema_version != 1 {
        return Err(LoadError::UnsupportedSchemaVersion(manifest.schema_version));
    }
    if !manifest.clips.contains_key("idle") {
        return Err(LoadError::MissingIdleClip);
    }

    let mut clips = HashMap::new();
    for (clip_name, stem) in manifest.clips {
        if stem.contains("..") || Path::new(&stem).is_absolute() {
            return Err(LoadError::PathEscapesPackage(stem));
        }
        let file_path = dir.join(format!("{stem}.png"));
        if !file_path.exists() {
            return Err(LoadError::MissingClipFile {
                clip: clip_name,
                path: file_path,
            });
        }
        clips.insert(clip_name, file_path);
    }

    Ok(Avatar {
        hitbox: manifest.hitbox,
        clips,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_manifest(dir: &std::path::Path, json: &str) {
        fs::write(dir.join("manifest.json"), json).unwrap();
    }

    #[test]
    fn loads_minimal_valid_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 130, "height": 133 },
                "clips": { "idle": "idle" }
            }"#,
        );
        fs::write(dir.path().join("idle.png"), b"fake-png-bytes").unwrap();

        let avatar = load(dir.path()).unwrap();
        assert_eq!(avatar.hitbox.width, 130.0);
        assert_eq!(avatar.hitbox.height, 133.0);
        assert_eq!(avatar.clips.get("idle").unwrap(), &dir.path().join("idle.png"));
    }

    #[test]
    fn rejects_missing_idle_clip() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "walk": "walk" }
            }"#,
        );
        fs::write(dir.path().join("walk.png"), b"fake").unwrap();

        assert!(matches!(load(dir.path()), Err(LoadError::MissingIdleClip)));
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 2,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "idle": "idle" }
            }"#,
        );
        fs::write(dir.path().join("idle.png"), b"fake").unwrap();

        assert!(matches!(
            load(dir.path()),
            Err(LoadError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn rejects_path_traversal_in_clip_stem() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "idle": "../escape" }
            }"#,
        );

        assert!(matches!(load(dir.path()), Err(LoadError::PathEscapesPackage(_))));
    }

    #[test]
    fn rejects_missing_clip_file() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "idle": "idle" }
            }"#,
        );
        // idle.png intentionally not written

        assert!(matches!(
            load(dir.path()),
            Err(LoadError::MissingClipFile { .. })
        ));
    }
}
