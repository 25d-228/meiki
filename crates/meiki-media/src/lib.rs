//! Validated local content-addressed media storage.
//!
//! Object paths are derived only from canonical SHA-256 values. Original file
//! names remain display metadata and never participate in filesystem paths.

use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tempfile::{Builder, NamedTempFile};
use thiserror::Error;

const HASH_PREFIX: &str = "sha256:";
const MAX_MEDIA_BYTES: u64 = 256 * 1024 * 1024;

/// Coarse media category determined from file signatures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectedMediaKind {
    Audio,
    Image,
}

/// Metadata produced by a successful import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedMedia {
    pub content_hash: String,
    pub kind: DetectedMediaKind,
    pub media_type: String,
    pub byte_size: u64,
    pub original_file_name: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub object_path: PathBuf,
    pub deduplicated: bool,
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("media file exceeds the {MAX_MEDIA_BYTES} byte safety limit")]
    FileTooLarge,
    #[error("media type is missing, unsupported, or unsafe")]
    UnsupportedFormat,
    #[error("media hash is not a canonical SHA-256 value: {0}")]
    InvalidHash(String),
    #[error("media object is missing: {0}")]
    MissingObject(String),
    #[error("media object checksum does not match {0}")]
    ChecksumMismatch(String),
    #[error("destination already exists: {}", .0.display())]
    DestinationExists(PathBuf),
    #[error("media filesystem operation {operation} failed for {}: {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Clone, Debug)]
pub struct MediaStore {
    root: PathBuf,
}

impl MediaStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Imports a supported audio or image after inspecting its file signature.
    ///
    /// The object is written atomically and identical bytes reuse the existing
    /// object path.
    ///
    /// # Errors
    ///
    /// Returns [`MediaError`] for unsupported input, oversized files, corrupt
    /// existing objects, or filesystem failures.
    pub fn import_file(&self, source: &Path) -> Result<ImportedMedia, MediaError> {
        let metadata =
            fs::metadata(source).map_err(|error| media_io("read metadata", source, error))?;
        if !metadata.is_file() {
            return Err(MediaError::UnsupportedFormat);
        }
        if metadata.len() > MAX_MEDIA_BYTES {
            return Err(MediaError::FileTooLarge);
        }

        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len()).map_err(|_| MediaError::FileTooLarge)?,
        );
        File::open(source)
            .map_err(|error| media_io("open", source, error))?
            .take(MAX_MEDIA_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| media_io("read", source, error))?;
        if u64::try_from(bytes.len()).map_or(true, |length| length > MAX_MEDIA_BYTES) {
            return Err(MediaError::FileTooLarge);
        }

        let detected = infer::get(&bytes).ok_or(MediaError::UnsupportedFormat)?;
        let (kind, media_type) = supported_media_type(detected.mime_type())?;
        let content_hash = content_hash(&bytes);
        let destination = self.object_path(&content_hash)?;
        let deduplicated = if destination.exists() {
            self.verify(&content_hash)?;
            true
        } else {
            write_object_atomically(&destination, &bytes)?;
            false
        };
        let (width, height) = image_dimensions(media_type, &bytes).unwrap_or((None, None));
        let duration_ms = audio_duration_ms(media_type, &bytes);
        Ok(ImportedMedia {
            content_hash,
            kind,
            media_type: media_type.to_owned(),
            byte_size: u64::try_from(bytes.len()).map_err(|_| MediaError::FileTooLarge)?,
            original_file_name: sanitized_file_name(source),
            width,
            height,
            duration_ms,
            object_path: destination,
            deduplicated,
        })
    }

    /// Resolves and verifies an object before returning its local path.
    ///
    /// # Errors
    ///
    /// Returns [`MediaError::MissingObject`] or
    /// [`MediaError::ChecksumMismatch`] when the object is unavailable.
    pub fn resolve(&self, hash: &str) -> Result<PathBuf, MediaError> {
        self.verify(hash)
    }

    /// Verifies one object and returns its safe content-addressed path.
    ///
    /// # Errors
    ///
    /// Returns an error when the hash is invalid, the object is missing, or
    /// its bytes do not match.
    pub fn verify(&self, hash: &str) -> Result<PathBuf, MediaError> {
        let path = self.object_path(hash)?;
        if !path.is_file() {
            return Err(MediaError::MissingObject(hash.to_owned()));
        }
        let actual = hash_file(&path)?;
        if actual != hash {
            return Err(MediaError::ChecksumMismatch(hash.to_owned()));
        }
        Ok(path)
    }

    /// Verifies every object stored below this root.
    ///
    /// # Errors
    ///
    /// Returns the first invalid path, checksum, or filesystem error.
    pub fn verify_all(&self) -> Result<Vec<String>, MediaError> {
        let hashes = self.stored_hashes()?;
        for hash in &hashes {
            self.verify(hash)?;
        }
        Ok(hashes)
    }

    /// Exports one verified object without exposing its internal object path.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination exists or verification/copy fails.
    pub fn export_object(&self, hash: &str, destination: &Path) -> Result<(), MediaError> {
        if destination.exists() {
            return Err(MediaError::DestinationExists(destination.to_path_buf()));
        }
        let source = self.verify(hash)?;
        copy_atomically(&source, destination)
    }

    /// Creates a checksum-verified media-store backup at a new path.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination exists or any object is invalid.
    pub fn backup_to(&self, destination: &Path) -> Result<(), MediaError> {
        if destination.exists() {
            return Err(MediaError::DestinationExists(destination.to_path_buf()));
        }
        let hashes = self.verify_all()?;
        copy_store_atomically(self, &hashes, destination)
    }

    /// Restores a checksum-verified media backup into a new store path.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination exists or the backup is invalid.
    pub fn restore_from_backup(backup: &Path, destination: &Path) -> Result<Self, MediaError> {
        if destination.exists() {
            return Err(MediaError::DestinationExists(destination.to_path_buf()));
        }
        let backup_store = Self::new(backup);
        let hashes = backup_store.verify_all()?;
        copy_store_atomically(&backup_store, &hashes, destination)?;
        let restored = Self::new(destination);
        restored.verify_all()?;
        Ok(restored)
    }

    /// Verifies a backup and restores any missing objects into this store.
    ///
    /// Existing objects are checksum-verified and never overwritten. Extra
    /// objects in the current store are preserved because content-addressed
    /// media is immutable and may support another recovery point.
    ///
    /// # Errors
    ///
    /// Returns an error when either store is corrupt or an object cannot be
    /// copied atomically.
    pub fn merge_from_backup(&self, backup: &Path) -> Result<(), MediaError> {
        let backup_store = Self::new(backup);
        let hashes = backup_store.verify_all()?;
        for hash in hashes {
            match self.verify(&hash) {
                Ok(_) => {}
                Err(MediaError::MissingObject(_)) => {
                    let source = backup_store.verify(&hash)?;
                    let destination = self.object_path(&hash)?;
                    copy_atomically(&source, &destination)?;
                    self.verify(&hash)?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Removes one verified, unreferenced object.
    ///
    /// Reference checks belong to the application/storage transaction and
    /// must happen before this filesystem operation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, missing, corrupt, or undeletable objects.
    pub fn remove(&self, hash: &str) -> Result<(), MediaError> {
        let path = self.verify(hash)?;
        fs::remove_file(&path).map_err(|error| media_io("remove", &path, error))
    }

    fn object_path(&self, hash: &str) -> Result<PathBuf, MediaError> {
        let digest = canonical_digest(hash)?;
        Ok(self
            .root
            .join("objects")
            .join("sha256")
            .join(&digest[..2])
            .join(&digest[2..]))
    }

    fn stored_hashes(&self) -> Result<Vec<String>, MediaError> {
        let algorithm_root = self.root.join("objects").join("sha256");
        if !algorithm_root.exists() {
            return Ok(Vec::new());
        }
        let mut hashes = Vec::new();
        for prefix in read_directory(&algorithm_root)? {
            let prefix_path = prefix.path();
            if !prefix_path.is_dir() {
                return Err(MediaError::InvalidHash(
                    prefix.file_name().to_string_lossy().into_owned(),
                ));
            }
            let prefix_name = prefix.file_name().to_string_lossy().into_owned();
            for object in read_directory(&prefix_path)? {
                if !object.path().is_file() {
                    return Err(MediaError::InvalidHash(
                        object.file_name().to_string_lossy().into_owned(),
                    ));
                }
                let digest = format!("{prefix_name}{}", object.file_name().to_string_lossy());
                canonical_digest(&format!("{HASH_PREFIX}{digest}"))?;
                hashes.push(format!("{HASH_PREFIX}{digest}"));
            }
        }
        hashes.sort_unstable();
        Ok(hashes)
    }
}

fn copy_store_atomically(
    source: &MediaStore,
    hashes: &[String],
    destination: &Path,
) -> Result<(), MediaError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| media_io("create parent", parent, error))?;
    let temporary = Builder::new()
        .prefix(".meiki-media-")
        .tempdir_in(parent)
        .map_err(|error| media_io("create temporary backup", parent, error))?;
    let temporary_store = MediaStore::new(temporary.path());
    for hash in hashes {
        let from = source.verify(hash)?;
        let to = temporary_store.object_path(hash)?;
        copy_atomically(&from, &to)?;
        temporary_store.verify(hash)?;
    }
    let temporary_path = temporary.keep();
    fs::rename(&temporary_path, destination)
        .map_err(|error| media_io("commit backup", destination, error))?;
    Ok(())
}

fn copy_atomically(source: &Path, destination: &Path) -> Result<(), MediaError> {
    let parent = destination.parent().ok_or_else(|| {
        media_io(
            "resolve destination parent",
            destination,
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent"),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| media_io("create directory", parent, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| media_io("create temporary file", parent, error))?;
    let mut input = File::open(source).map_err(|error| media_io("open", source, error))?;
    io::copy(&mut input, &mut temporary).map_err(|error| media_io("copy", destination, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| media_io("sync", destination, error))?;
    temporary
        .persist_noclobber(destination)
        .map_err(|error| media_io("persist", destination, error.error))?;
    Ok(())
}

fn write_object_atomically(destination: &Path, bytes: &[u8]) -> Result<(), MediaError> {
    let parent = destination.parent().ok_or_else(|| {
        media_io(
            "resolve object parent",
            destination,
            io::Error::new(io::ErrorKind::InvalidInput, "object has no parent"),
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| media_io("create object directory", parent, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| media_io("create temporary object", parent, error))?;
    temporary
        .write_all(bytes)
        .map_err(|error| media_io("write object", destination, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| media_io("sync object", destination, error))?;
    match temporary.persist_noclobber(destination) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            let expected = content_hash(bytes);
            let actual = hash_file(destination)?;
            if actual == expected {
                Ok(())
            } else {
                Err(MediaError::ChecksumMismatch(expected))
            }
        }
        Err(error) => Err(media_io("persist object", destination, error.error)),
    }
}

fn supported_media_type(media_type: &str) -> Result<(DetectedMediaKind, &'static str), MediaError> {
    match media_type {
        "image/png" => Ok((DetectedMediaKind::Image, "image/png")),
        "image/jpeg" => Ok((DetectedMediaKind::Image, "image/jpeg")),
        "image/gif" => Ok((DetectedMediaKind::Image, "image/gif")),
        "image/webp" => Ok((DetectedMediaKind::Image, "image/webp")),
        "audio/mpeg" => Ok((DetectedMediaKind::Audio, "audio/mpeg")),
        "audio/m4a" => Ok((DetectedMediaKind::Audio, "audio/mp4")),
        "audio/opus" | "audio/ogg" => Ok((DetectedMediaKind::Audio, "audio/ogg")),
        "audio/x-flac" => Ok((DetectedMediaKind::Audio, "audio/flac")),
        "audio/x-wav" => Ok((DetectedMediaKind::Audio, "audio/wav")),
        "audio/aac" => Ok((DetectedMediaKind::Audio, "audio/aac")),
        _ => Err(MediaError::UnsupportedFormat),
    }
}

fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{HASH_PREFIX}{}", hex::encode(digest))
}

fn hash_file(path: &Path) -> Result<String, MediaError> {
    let mut file = File::open(path).map_err(|error| media_io("open object", path, error))?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| media_io("read object", path, error))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{HASH_PREFIX}{}", hex::encode(digest.finalize())))
}

fn canonical_digest(hash: &str) -> Result<&str, MediaError> {
    let Some(digest) = hash.strip_prefix(HASH_PREFIX) else {
        return Err(MediaError::InvalidHash(hash.to_owned()));
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(MediaError::InvalidHash(hash.to_owned()));
    }
    Ok(digest)
}

fn sanitized_file_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("media");
    let mut sanitized = raw
        .chars()
        .filter(|character| !character.is_control())
        .take(255)
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "media".clone_into(&mut sanitized);
    }
    sanitized
}

fn image_dimensions(media_type: &str, bytes: &[u8]) -> Option<(Option<u32>, Option<u32>)> {
    let dimensions = match media_type {
        "image/png" if bytes.len() >= 24 => Some((
            u32::from_be_bytes(bytes[16..20].try_into().ok()?),
            u32::from_be_bytes(bytes[20..24].try_into().ok()?),
        )),
        "image/gif" if bytes.len() >= 10 => Some((
            u32::from(u16::from_le_bytes(bytes[6..8].try_into().ok()?)),
            u32::from(u16::from_le_bytes(bytes[8..10].try_into().ok()?)),
        )),
        "image/jpeg" => jpeg_dimensions(bytes),
        _ => None,
    };
    dimensions.map(|(width, height)| (Some(width), Some(height)))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut cursor = 2;
    while cursor + 4 < bytes.len() {
        if bytes[cursor] != 0xff {
            cursor += 1;
            continue;
        }
        while cursor < bytes.len() && bytes[cursor] == 0xff {
            cursor += 1;
        }
        let marker = *bytes.get(cursor)?;
        cursor += 1;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(
            bytes.get(cursor..cursor + 2)?.try_into().ok()?,
        ));
        if length < 2 || cursor + length > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            let height = u32::from(u16::from_be_bytes(
                bytes.get(cursor + 3..cursor + 5)?.try_into().ok()?,
            ));
            let width = u32::from(u16::from_be_bytes(
                bytes.get(cursor + 5..cursor + 7)?.try_into().ok()?,
            ));
            return Some((width, height));
        }
        cursor += length;
    }
    None
}

fn audio_duration_ms(media_type: &str, bytes: &[u8]) -> Option<u64> {
    if media_type != "audio/wav" || bytes.len() < 44 {
        return None;
    }
    let byte_rate = u64::from(u32::from_le_bytes(bytes.get(28..32)?.try_into().ok()?));
    if byte_rate == 0 {
        return None;
    }
    let mut cursor = 12;
    while cursor + 8 <= bytes.len() {
        let chunk = bytes.get(cursor..cursor + 4)?;
        let size = usize::try_from(u32::from_le_bytes(
            bytes.get(cursor + 4..cursor + 8)?.try_into().ok()?,
        ))
        .ok()?;
        if chunk == b"data" {
            return u64::try_from(size)
                .ok()?
                .checked_mul(1_000)?
                .checked_div(byte_rate);
        }
        cursor = cursor.checked_add(8 + size + (size % 2))?;
    }
    None
}

fn read_directory(path: &Path) -> Result<Vec<fs::DirEntry>, MediaError> {
    fs::read_dir(path)
        .map_err(|error| media_io("read directory", path, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| media_io("read directory entry", path, error))
}

fn media_io(operation: &'static str, path: &Path, source: io::Error) -> MediaError {
    MediaError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use tempfile::tempdir;

    use super::{DetectedMediaKind, MediaError, MediaStore};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
        ];
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.extend([8, 6, 0, 0, 0]);
        bytes
    }

    fn wav(duration_ms: u32) -> Vec<u8> {
        let sample_rate = 8_000_u32;
        let bytes_per_sample = 2_u32;
        let data_size = sample_rate * bytes_per_sample * duration_ms / 1_000;
        let mut bytes = Vec::with_capacity(44 + usize::try_from(data_size).unwrap());
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_size).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(sample_rate.to_le_bytes());
        bytes.extend((sample_rate * bytes_per_sample).to_le_bytes());
        bytes.extend(u16::try_from(bytes_per_sample).unwrap().to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_size.to_le_bytes());
        bytes.resize(44 + usize::try_from(data_size).unwrap(), 0);
        bytes
    }

    #[test]
    fn import_deduplicates_resolves_exports_and_restores() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("図書館 👩🏽‍💻.png");
        fs::write(&source, png(320, 180)).unwrap();
        let store = MediaStore::new(directory.path().join("media"));

        let imported = store.import_file(&source).unwrap();
        assert_eq!(imported.kind, DetectedMediaKind::Image);
        assert_eq!(imported.media_type, "image/png");
        assert_eq!(imported.width, Some(320));
        assert_eq!(imported.height, Some(180));
        assert!(!imported.deduplicated);
        assert_eq!(
            store.resolve(&imported.content_hash).unwrap(),
            imported.object_path
        );

        let duplicate = store.import_file(&source).unwrap();
        assert!(duplicate.deduplicated);
        assert_eq!(duplicate.object_path, imported.object_path);
        assert_eq!(store.verify_all().unwrap().len(), 1);

        let exported = directory.path().join("exported.png");
        store
            .export_object(&imported.content_hash, &exported)
            .unwrap();
        assert_eq!(fs::read(exported).unwrap(), png(320, 180));

        let backup = directory.path().join("backup");
        store.backup_to(&backup).unwrap();
        let restored_path = directory.path().join("restored");
        let restored = MediaStore::restore_from_backup(&backup, &restored_path).unwrap();
        assert_eq!(
            restored.verify_all().unwrap(),
            vec![imported.content_hash.clone()]
        );

        let existing_path = directory.path().join("existing");
        let existing = MediaStore::new(&existing_path);
        existing.merge_from_backup(&backup).unwrap();
        existing.merge_from_backup(&backup).unwrap();
        assert_eq!(existing.verify_all().unwrap(), vec![imported.content_hash]);
    }

    #[test]
    fn unsafe_missing_and_corrupt_objects_fail_closed() {
        let directory = tempdir().unwrap();
        let disguised = directory.path().join("script.png");
        fs::write(&disguised, b"<script>alert(1)</script>").unwrap();
        let store = MediaStore::new(directory.path().join("media"));
        assert!(matches!(
            store.import_file(&disguised),
            Err(MediaError::UnsupportedFormat)
        ));
        assert!(matches!(
            store.resolve("../../escape"),
            Err(MediaError::InvalidHash(_))
        ));

        let source = directory.path().join("safe.png");
        fs::write(&source, png(1, 1)).unwrap();
        let imported = store.import_file(&source).unwrap();
        let mut object = fs::OpenOptions::new()
            .append(true)
            .open(&imported.object_path)
            .unwrap();
        object.write_all(b"corrupt").unwrap();
        assert!(matches!(
            store.resolve(&imported.content_hash),
            Err(MediaError::ChecksumMismatch(_))
        ));
        let export = directory.path().join("should-not-export.png");
        assert!(matches!(
            store.export_object(&imported.content_hash, &export),
            Err(MediaError::ChecksumMismatch(_))
        ));
        assert!(!export.exists());
    }

    #[test]
    fn restore_rejects_a_backup_with_a_changed_object() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("safe.png");
        fs::write(&source, png(2, 2)).unwrap();
        let store = MediaStore::new(directory.path().join("media"));
        let imported = store.import_file(&source).unwrap();
        let backup = directory.path().join("backup");
        store.backup_to(&backup).unwrap();

        let backup_store = MediaStore::new(&backup);
        let object_path = backup_store.object_path(&imported.content_hash).unwrap();
        let mut object = fs::OpenOptions::new()
            .append(true)
            .open(object_path)
            .unwrap();
        object.write_all(b"changed").unwrap();
        let restored = directory.path().join("restored");
        assert!(matches!(
            MediaStore::restore_from_backup(&backup, &restored),
            Err(MediaError::ChecksumMismatch(_))
        ));
        assert!(!restored.exists());
    }

    #[test]
    fn wav_import_uses_its_signature_and_records_duration() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("prompt.txt");
        fs::write(&source, wav(1_000)).unwrap();
        let store = MediaStore::new(directory.path().join("media"));
        let imported = store.import_file(&source).unwrap();
        assert_eq!(imported.kind, DetectedMediaKind::Audio);
        assert_eq!(imported.media_type, "audio/wav");
        assert_eq!(imported.duration_ms, Some(1_000));
        assert_eq!(imported.byte_size, 16_044);
    }
}
