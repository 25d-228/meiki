//! Versioned, validated `.meiki` collection archives.
//!
//! The archive stores canonical UTF-8 JSON and checksum-addressed media. It
//! never stores or extracts caller-controlled filesystem paths.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use meiki_domain::{
    Card, Cloze, Deck, ReviewEvent, ReviewEventKind, ScheduleState, SchedulerParameterSet,
    SchedulerProfile, SourceItem,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::{NamedTempFile, TempDir};
use thiserror::Error;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const ARCHIVE_FORMAT: &str = "meiki";
pub const ARCHIVE_VERSION: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const COLLECTION_ENTRY: &str = "collection.json";
const MAX_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_COLLECTION_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_002;
const HASH_PREFIX: &str = "sha256:";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveScope {
    FullCollection,
    SelectedDecks,
    SelectedNotes,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchiveCounts {
    pub decks: u64,
    pub notes: u64,
    pub cards: u64,
    pub review_events: u64,
    pub media_objects: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchiveMediaEntry {
    pub content_hash: String,
    pub path: String,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArchiveManifest {
    pub format: String,
    pub version: u32,
    pub created_at_ms: i64,
    pub scope: ArchiveScope,
    pub collection_path: String,
    pub collection_sha256: String,
    pub counts: ArchiveCounts,
    pub media: Vec<ArchiveMediaEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PortableCard {
    pub card: Card,
    pub baseline: ScheduleState,
    pub schedule: ScheduleState,
    pub review_events: Vec<ReviewEvent>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PortableNote {
    pub source_item: SourceItem,
    pub clozes: Vec<Cloze>,
    pub cards: Vec<PortableCard>,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PortableCollection {
    pub decks: Vec<Deck>,
    pub notes: Vec<PortableNote>,
    pub scheduler_parameter_sets: Vec<SchedulerParameterSet>,
    pub scheduler_profiles: Vec<SchedulerProfile>,
}

#[derive(Clone, Debug)]
pub struct ArchiveMediaSource {
    pub content_hash: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ValidatedMediaObject {
    pub content_hash: String,
    pub path: PathBuf,
    pub byte_size: u64,
}

#[derive(Debug)]
pub struct ValidatedArchive {
    pub manifest: ArchiveManifest,
    pub collection: PortableCollection,
    pub media_objects: Vec<ValidatedMediaObject>,
    _temporary: TempDir,
}

#[derive(Debug, Error)]
pub enum PortableError {
    #[error("archive destination already exists: {}", .0.display())]
    DestinationExists(PathBuf),
    #[error("archive exceeds the {MAX_ARCHIVE_BYTES} byte safety limit")]
    ArchiveTooLarge,
    #[error("archive contains too many entries")]
    TooManyEntries,
    #[error("archive entry is missing: {0}")]
    MissingEntry(String),
    #[error("archive contains a duplicate or unexpected entry: {0}")]
    UnexpectedEntry(String),
    #[error("archive manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("archive collection is invalid: {0}")]
    InvalidCollection(String),
    #[error("archive media object is invalid: {0}")]
    InvalidMedia(String),
    #[error("archive checksum does not match for {0}")]
    ChecksumMismatch(String),
    #[error("archive JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("archive container is invalid: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("archive filesystem operation {operation} failed for {}: {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Writes a complete checksum-verified `.meiki` archive without overwriting.
///
/// # Errors
///
/// Returns an error for invalid collection identities, missing or corrupt
/// media, unsafe sizes, an existing destination, or an I/O failure.
pub fn write_archive(
    destination: &Path,
    collection: &PortableCollection,
    media_sources: &[ArchiveMediaSource],
    scope: ArchiveScope,
    created_at_ms: i64,
) -> Result<ArchiveManifest, PortableError> {
    if destination.exists() {
        return Err(PortableError::DestinationExists(destination.to_path_buf()));
    }
    validate_collection(collection)?;
    let collection_json = serde_json::to_vec(collection)?;
    if portable_count(collection_json.len())? > MAX_COLLECTION_BYTES {
        return Err(PortableError::ArchiveTooLarge);
    }

    let referenced_hashes = referenced_media_hashes(collection)?;
    let mut by_hash = HashMap::new();
    for source in media_sources {
        canonical_digest(&source.content_hash)?;
        if by_hash
            .insert(source.content_hash.as_str(), source.path.as_path())
            .is_some()
        {
            return Err(PortableError::InvalidMedia(format!(
                "duplicate media source {}",
                source.content_hash
            )));
        }
    }
    if referenced_hashes.len() != by_hash.len()
        || !referenced_hashes
            .iter()
            .all(|hash| by_hash.contains_key(hash.as_str()))
    {
        return Err(PortableError::InvalidMedia(
            "media sources must exactly cover referenced content hashes".into(),
        ));
    }

    let mut media = Vec::with_capacity(referenced_hashes.len());
    for hash in &referenced_hashes {
        let path = by_hash[hash.as_str()];
        let metadata = fs::metadata(path).map_err(|error| portable_io("inspect", path, error))?;
        if !metadata.is_file() || metadata.len() > MAX_MEDIA_BYTES {
            return Err(PortableError::InvalidMedia(hash.clone()));
        }
        if hash_file(path)? != *hash {
            return Err(PortableError::ChecksumMismatch(hash.clone()));
        }
        media.push(ArchiveMediaEntry {
            content_hash: hash.clone(),
            path: media_entry_path(hash)?,
            byte_size: metadata.len(),
        });
    }
    media.sort_by(|left, right| left.content_hash.cmp(&right.content_hash));

    let counts = collection_counts(collection)?;
    let manifest = ArchiveManifest {
        format: ARCHIVE_FORMAT.into(),
        version: ARCHIVE_VERSION,
        created_at_ms,
        scope,
        collection_path: COLLECTION_ENTRY.into(),
        collection_sha256: content_hash(&collection_json),
        counts,
        media,
    };
    validate_media_manifest_alignment(collection, &manifest)?;
    let manifest_json = serde_json::to_vec(&manifest)?;

    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| portable_io("create directory", parent, error))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| portable_io("create temporary archive", parent, error))?;
    {
        let mut writer = ZipWriter::new(temporary.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        write_zip_bytes(&mut writer, MANIFEST_ENTRY, &manifest_json, options)?;
        write_zip_bytes(&mut writer, COLLECTION_ENTRY, &collection_json, options)?;
        for entry in &manifest.media {
            let source = by_hash[entry.content_hash.as_str()];
            writer.start_file(&entry.path, options)?;
            let mut file =
                File::open(source).map_err(|error| portable_io("open media", source, error))?;
            io::copy(&mut file, &mut writer)
                .map_err(|error| portable_io("write media", destination, error))?;
        }
        writer.finish()?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| portable_io("sync archive", destination, error))?;
    if temporary
        .as_file()
        .metadata()
        .map_or(true, |value| value.len() > MAX_ARCHIVE_BYTES)
    {
        return Err(PortableError::ArchiveTooLarge);
    }
    temporary
        .persist_noclobber(destination)
        .map_err(|error| portable_io("persist archive", destination, error.error))?;
    Ok(manifest)
}

/// Reads, bounds-checks, and checksum-verifies an archive into temporary files.
///
/// No archive-provided path is ever passed to the filesystem.
///
/// # Errors
///
/// Returns an error before exposing collection data when any entry, identity,
/// relationship, size, or checksum is invalid.
pub fn read_archive(path: &Path) -> Result<ValidatedArchive, PortableError> {
    let metadata = fs::metadata(path).map_err(|error| portable_io("inspect", path, error))?;
    if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(PortableError::ArchiveTooLarge);
    }
    let file = File::open(path).map_err(|error| portable_io("open", path, error))?;
    let mut archive = ZipArchive::new(file)?;
    if archive.len() > MAX_ENTRIES {
        return Err(PortableError::TooManyEntries);
    }
    let names = archive_names(&mut archive)?;
    let manifest_bytes = read_entry(&mut archive, MANIFEST_ENTRY, MAX_COLLECTION_BYTES)?;
    let manifest: ArchiveManifest = serde_json::from_slice(&manifest_bytes)?;
    validate_manifest(&manifest)?;

    let expected = std::iter::once(MANIFEST_ENTRY.to_owned())
        .chain(std::iter::once(manifest.collection_path.clone()))
        .chain(manifest.media.iter().map(|entry| entry.path.clone()))
        .collect::<HashSet<_>>();
    if names != expected {
        let unexpected = names
            .symmetric_difference(&expected)
            .next()
            .cloned()
            .unwrap_or_else(|| "duplicate entry".into());
        return Err(PortableError::UnexpectedEntry(unexpected));
    }

    let collection_bytes = read_entry(
        &mut archive,
        &manifest.collection_path,
        MAX_COLLECTION_BYTES,
    )?;
    if content_hash(&collection_bytes) != manifest.collection_sha256 {
        return Err(PortableError::ChecksumMismatch(
            manifest.collection_path.clone(),
        ));
    }
    let collection: PortableCollection = serde_json::from_slice(&collection_bytes)?;
    validate_collection(&collection)?;
    if collection_counts(&collection)? != manifest.counts {
        return Err(PortableError::InvalidManifest(
            "manifest counts do not match collection data".into(),
        ));
    }
    validate_media_manifest_alignment(&collection, &manifest)?;

    let temporary =
        tempfile::tempdir().map_err(|error| portable_io("create import workspace", path, error))?;
    let mut media_objects = Vec::with_capacity(manifest.media.len());
    for (index, entry) in manifest.media.iter().enumerate() {
        let output = temporary.path().join(index.to_string());
        extract_media_entry(&mut archive, entry, &output)?;
        media_objects.push(ValidatedMediaObject {
            content_hash: entry.content_hash.clone(),
            path: output,
            byte_size: entry.byte_size,
        });
    }
    Ok(ValidatedArchive {
        manifest,
        collection,
        media_objects,
        _temporary: temporary,
    })
}

/// Validates collection identity and history relationships.
///
/// # Errors
///
/// Returns a descriptive validation error for duplicate IDs, broken
/// references, or inconsistent schedule history.
#[allow(clippy::too_many_lines)]
pub fn validate_collection(collection: &PortableCollection) -> Result<(), PortableError> {
    if collection.decks.is_empty() {
        return invalid_collection("a portable collection must contain at least one deck");
    }
    unique_ids(collection.decks.iter().map(|deck| deck.id.as_str()), "deck")?;
    unique_ids(
        collection
            .scheduler_parameter_sets
            .iter()
            .map(|set| set.id.as_str()),
        "scheduler parameter set",
    )?;
    unique_ids(
        collection
            .notes
            .iter()
            .map(|note| note.source_item.id.as_str()),
        "source note",
    )?;
    let deck_ids = collection
        .decks
        .iter()
        .map(|deck| deck.id.as_str())
        .collect::<HashSet<_>>();
    let parameter_ids = collection
        .scheduler_parameter_sets
        .iter()
        .map(|set| set.id.as_str())
        .collect::<HashSet<_>>();
    for profile in &collection.scheduler_profiles {
        if !deck_ids.contains(profile.deck_id.as_str())
            || !parameter_ids.contains(profile.active_parameter_set_id.as_str())
            || profile
                .previous_parameter_set_id
                .as_ref()
                .is_some_and(|id| !parameter_ids.contains(id.as_str()))
        {
            return invalid_collection("scheduler profile references missing data");
        }
    }
    let profile_deck_ids = collection
        .scheduler_profiles
        .iter()
        .map(|profile| profile.deck_id.as_str())
        .collect::<HashSet<_>>();
    if profile_deck_ids != deck_ids {
        return invalid_collection("each deck must have exactly one scheduler profile");
    }
    unique_ids(
        collection
            .scheduler_profiles
            .iter()
            .map(|profile| profile.deck_id.as_str()),
        "scheduler profile",
    )?;

    let mut global_ids = HashSet::new();
    let mut annotation_ids = HashSet::new();
    let mut tags = HashMap::new();
    let mut media_references = HashMap::new();
    let mut review_ids = HashSet::new();
    for note in &collection.notes {
        let source = &note.source_item;
        if !deck_ids.contains(source.deck_id.as_str())
            || note.clozes.len() != note.cards.len()
            || source.segments.is_empty()
        {
            return invalid_collection("source note relationships are incomplete");
        }
        unique_ids(
            source.segments.iter().map(|segment| segment.id.as_str()),
            "segment",
        )?;
        unique_ids(note.clozes.iter().map(|cloze| cloze.id.as_str()), "cloze")?;
        unique_ids(note.cards.iter().map(|card| card.card.id.as_str()), "card")?;
        let cloze_ids = note
            .clozes
            .iter()
            .map(|cloze| cloze.id.as_str())
            .collect::<HashSet<_>>();
        let card_cloze_ids = note
            .cards
            .iter()
            .map(|card| card.card.cloze_id.as_str())
            .collect::<HashSet<_>>();
        if card_cloze_ids != cloze_ids {
            return invalid_collection("each cloze must own exactly one card");
        }
        let segment_cloze_ids = source
            .segments
            .iter()
            .filter_map(|segment| match &segment.content {
                meiki_domain::SegmentContent::Text(_) => None,
                meiki_domain::SegmentContent::Cloze { cloze_id, .. } => Some(cloze_id.as_str()),
            })
            .collect::<HashSet<_>>();
        let segment_cloze_count = source
            .segments
            .iter()
            .filter(|segment| {
                matches!(&segment.content, meiki_domain::SegmentContent::Cloze { .. })
            })
            .count();
        if segment_cloze_ids != cloze_ids
            || segment_cloze_count != cloze_ids.len()
            || source
                .segments
                .iter()
                .enumerate()
                .any(|(ordinal, segment)| usize::try_from(segment.ordinal) != Ok(ordinal))
        {
            return invalid_collection(
                "segments must be contiguous and contain each cloze exactly once",
            );
        }
        for cloze in &note.clozes {
            if cloze.source_item_id != source.id {
                return invalid_collection("cloze references the wrong source note");
            }
        }
        for segment in &source.segments {
            if let meiki_domain::SegmentContent::Cloze { cloze_id, .. } = &segment.content
                && !cloze_ids.contains(cloze_id.as_str())
            {
                return invalid_collection("segment references a missing cloze");
            }
        }
        for portable in &note.cards {
            if !cloze_ids.contains(portable.card.cloze_id.as_str()) {
                return invalid_collection("card references a missing cloze");
            }
            validate_card_history(portable, &parameter_ids, &mut review_ids)?;
        }
        for id in std::iter::once(source.id.as_str())
            .chain(source.segments.iter().map(|value| value.id.as_str()))
            .chain(note.clozes.iter().map(|value| value.id.as_str()))
            .chain(note.cards.iter().map(|value| value.card.id.as_str()))
        {
            if !global_ids.insert(id) {
                return invalid_collection("entity IDs must be globally unique");
            }
        }
        for annotation in source.annotations.iter().chain(
            note.clozes
                .iter()
                .flat_map(|cloze| cloze.annotations.iter()),
        ) {
            if annotation.id.trim().is_empty() || !annotation_ids.insert(annotation.id.as_str()) {
                return invalid_collection("annotation IDs must be non-empty and globally unique");
            }
        }
        for tag in &source.tags {
            if tag.id.trim().is_empty() {
                return invalid_collection("tag IDs must be non-empty");
            }
            if let Some(existing) = tags.insert(tag.id.as_str(), tag)
                && existing != tag
            {
                return invalid_collection("a reused tag ID has conflicting content");
            }
        }
        for media in source
            .media
            .iter()
            .chain(note.clozes.iter().flat_map(|cloze| cloze.media.iter()))
        {
            if media.id.trim().is_empty() {
                return invalid_collection("media reference IDs must be non-empty");
            }
            if let Some(existing) = media_references.insert(media.id.as_str(), media)
                && existing != media
            {
                return invalid_collection("a reused media reference ID has conflicting metadata");
            }
        }
    }
    referenced_media_hashes(collection)?;
    Ok(())
}

fn validate_card_history(
    portable: &PortableCard,
    parameter_ids: &HashSet<&str>,
    review_ids: &mut HashSet<String>,
) -> Result<(), PortableError> {
    let card_id = portable.card.id.as_str();
    if portable.baseline.card_id != card_id
        || portable.baseline.version != 0
        || portable.baseline.last_review_event_id.is_some()
        || portable.schedule.card_id != card_id
    {
        return invalid_collection("card schedule references the wrong card");
    }
    let mut projected = portable.baseline.clone();
    let mut active_review_ids = HashSet::new();
    for event in &portable.review_events {
        if event.card_id != card_id
            || event.previous_schedule != projected
            || event.next_schedule.card_id != card_id
            || event.next_schedule.version != event.previous_schedule.version + 1
            || event.next_schedule.last_review_event_id.as_deref() != Some(event.id.as_str())
            || !review_ids.insert(event.id.clone())
            || event
                .scheduler_parameter_set_id
                .as_ref()
                .is_some_and(|id| !parameter_ids.contains(id.as_str()))
        {
            return invalid_collection("review history projection chain is invalid");
        }
        match event.kind {
            ReviewEventKind::Review if event.undoes_review_event_id.is_none() => {
                active_review_ids.insert(event.id.as_str());
            }
            ReviewEventKind::Undo => {
                let Some(target) = event.undoes_review_event_id.as_deref() else {
                    return invalid_collection("undo event has no review target");
                };
                if !active_review_ids.remove(target) {
                    return invalid_collection("undo event target is not an active review");
                }
            }
            ReviewEventKind::Review => {
                return invalid_collection("review event cannot undo another review");
            }
        }
        projected = event.next_schedule.clone();
    }
    if projected != portable.schedule {
        return invalid_collection("current schedule does not match review history");
    }
    Ok(())
}

/// Deterministically namespaces every database identity in a collection.
///
/// Media content hashes are intentionally unchanged so an import can
/// deduplicate identical objects by checksum.
///
/// # Errors
///
/// Returns an error when `fingerprint` is not a canonical SHA-256 value or
/// the remapped collection is invalid.
pub fn namespace_collection(
    collection: &PortableCollection,
    fingerprint: &str,
) -> Result<PortableCollection, PortableError> {
    let digest = canonical_digest(fingerprint)?;
    let prefix = format!("import-{}-", &digest[..16]);
    let map_id = |id: &str| format!("{prefix}{id}");
    let mut mapped = collection.clone();

    for deck in &mut mapped.decks {
        deck.id = map_id(&deck.id);
    }
    for parameter_set in &mut mapped.scheduler_parameter_sets {
        parameter_set.id = map_id(&parameter_set.id);
    }
    for profile in &mut mapped.scheduler_profiles {
        profile.deck_id = map_id(&profile.deck_id);
        profile.active_parameter_set_id = map_id(&profile.active_parameter_set_id);
        profile.previous_parameter_set_id =
            profile.previous_parameter_set_id.as_deref().map(&map_id);
    }
    for note in &mut mapped.notes {
        note.source_item.id = map_id(&note.source_item.id);
        note.source_item.deck_id = map_id(&note.source_item.deck_id);
        for segment in &mut note.source_item.segments {
            segment.id = map_id(&segment.id);
            if let meiki_domain::SegmentContent::Cloze { cloze_id, .. } = &mut segment.content {
                *cloze_id = map_id(cloze_id);
            }
        }
        for tag in &mut note.source_item.tags {
            tag.id = map_id(&tag.id);
        }
        for annotation in &mut note.source_item.annotations {
            annotation.id = map_id(&annotation.id);
        }
        for media in &mut note.source_item.media {
            media.id = map_id(&media.id);
        }
        for cloze in &mut note.clozes {
            cloze.id = map_id(&cloze.id);
            cloze.source_item_id = map_id(&cloze.source_item_id);
            for annotation in &mut cloze.annotations {
                annotation.id = map_id(&annotation.id);
            }
            for media in &mut cloze.media {
                media.id = map_id(&media.id);
            }
        }
        for portable in &mut note.cards {
            portable.card.id = map_id(&portable.card.id);
            portable.card.cloze_id = map_id(&portable.card.cloze_id);
            map_schedule(&mut portable.baseline, &map_id);
            map_schedule(&mut portable.schedule, &map_id);
            for event in &mut portable.review_events {
                event.id = map_id(&event.id);
                event.card_id = map_id(&event.card_id);
                event.undoes_review_event_id = event.undoes_review_event_id.as_deref().map(&map_id);
                event.scheduler_parameter_set_id =
                    event.scheduler_parameter_set_id.as_deref().map(&map_id);
                map_schedule(&mut event.previous_schedule, &map_id);
                map_schedule(&mut event.next_schedule, &map_id);
            }
        }
    }
    validate_collection(&mapped)?;
    Ok(mapped)
}

fn map_schedule(schedule: &mut ScheduleState, map_id: &impl Fn(&str) -> String) {
    schedule.card_id = map_id(&schedule.card_id);
    schedule.last_review_event_id = schedule.last_review_event_id.as_deref().map(map_id);
}

fn referenced_media_hashes(collection: &PortableCollection) -> Result<Vec<String>, PortableError> {
    let mut hashes = HashSet::new();
    for media in collection.notes.iter().flat_map(|note| {
        note.source_item
            .media
            .iter()
            .chain(note.clozes.iter().flat_map(|cloze| cloze.media.iter()))
    }) {
        canonical_digest(&media.content_hash)?;
        hashes.insert(media.content_hash.clone());
    }
    let mut hashes = hashes.into_iter().collect::<Vec<_>>();
    hashes.sort_unstable();
    Ok(hashes)
}

fn validate_media_manifest_alignment(
    collection: &PortableCollection,
    manifest: &ArchiveManifest,
) -> Result<(), PortableError> {
    let sizes = manifest
        .media
        .iter()
        .map(|entry| (entry.content_hash.as_str(), entry.byte_size))
        .collect::<HashMap<_, _>>();
    let mut technical_metadata = HashMap::new();
    for media in collection.notes.iter().flat_map(|note| {
        note.source_item
            .media
            .iter()
            .chain(note.clozes.iter().flat_map(|cloze| cloze.media.iter()))
    }) {
        if sizes.get(media.content_hash.as_str()) != Some(&media.byte_size) {
            return Err(PortableError::InvalidMedia(format!(
                "{} has inconsistent byte size metadata",
                media.content_hash
            )));
        }
        let metadata = (
            media.kind,
            media.media_type.as_str(),
            media.byte_size,
            media.width,
            media.height,
            media.duration_ms,
        );
        if let Some(existing) = technical_metadata.insert(media.content_hash.as_str(), metadata)
            && existing != metadata
        {
            return Err(PortableError::InvalidMedia(format!(
                "{} has conflicting technical metadata",
                media.content_hash
            )));
        }
    }
    Ok(())
}

fn collection_counts(collection: &PortableCollection) -> Result<ArchiveCounts, PortableError> {
    let cards = collection
        .notes
        .iter()
        .map(|note| note.cards.len())
        .sum::<usize>();
    let reviews = collection
        .notes
        .iter()
        .flat_map(|note| &note.cards)
        .map(|card| card.review_events.len())
        .sum::<usize>();
    Ok(ArchiveCounts {
        decks: portable_count(collection.decks.len())?,
        notes: portable_count(collection.notes.len())?,
        cards: portable_count(cards)?,
        review_events: portable_count(reviews)?,
        media_objects: portable_count(referenced_media_hashes(collection)?.len())?,
    })
}

fn validate_manifest(manifest: &ArchiveManifest) -> Result<(), PortableError> {
    if manifest.format != ARCHIVE_FORMAT
        || manifest.version != ARCHIVE_VERSION
        || manifest.collection_path != COLLECTION_ENTRY
        || manifest.created_at_ms < 0
    {
        return Err(PortableError::InvalidManifest(
            "format, version, path, or timestamp is unsupported".into(),
        ));
    }
    if portable_count(manifest.media.len())? != manifest.counts.media_objects {
        return Err(PortableError::InvalidManifest(
            "media count does not match the manifest inventory".into(),
        ));
    }
    canonical_digest(&manifest.collection_sha256)
        .map_err(|_| PortableError::InvalidManifest("invalid collection checksum".into()))?;
    unique_ids(
        manifest.media.iter().map(|entry| entry.path.as_str()),
        "media archive path",
    )?;
    unique_ids(
        manifest
            .media
            .iter()
            .map(|entry| entry.content_hash.as_str()),
        "media content hash",
    )?;
    for entry in &manifest.media {
        if entry.path != media_entry_path(&entry.content_hash)? || entry.byte_size > MAX_MEDIA_BYTES
        {
            return Err(PortableError::InvalidManifest(
                "media path or size is unsafe".into(),
            ));
        }
    }
    Ok(())
}

fn extract_media_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    entry: &ArchiveMediaEntry,
    output: &Path,
) -> Result<(), PortableError> {
    let mut input = archive.by_name(&entry.path)?;
    if input.is_dir() || input.size() != entry.byte_size || input.size() > MAX_MEDIA_BYTES {
        return Err(PortableError::InvalidMedia(entry.content_hash.clone()));
    }
    let mut destination =
        File::create(output).map_err(|error| portable_io("create media", output, error))?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| portable_io("read media entry", output, error))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(portable_count(count)?)
            .ok_or(PortableError::ArchiveTooLarge)?;
        if total > entry.byte_size || total > MAX_MEDIA_BYTES {
            return Err(PortableError::InvalidMedia(entry.content_hash.clone()));
        }
        digest.update(&buffer[..count]);
        destination
            .write_all(&buffer[..count])
            .map_err(|error| portable_io("write media", output, error))?;
    }
    if total != entry.byte_size
        || format!("{HASH_PREFIX}{}", hex::encode(digest.finalize())) != entry.content_hash
    {
        return Err(PortableError::ChecksumMismatch(entry.content_hash.clone()));
    }
    destination
        .sync_all()
        .map_err(|error| portable_io("sync media", output, error))
}

fn archive_names<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<HashSet<String>, PortableError> {
    let mut names = HashSet::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        if entry.is_dir() || !names.insert(name.clone()) {
            return Err(PortableError::UnexpectedEntry(name));
        }
    }
    Ok(names)
}

fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, PortableError> {
    let entry = archive.by_name(name).map_err(|error| match error {
        zip::result::ZipError::FileNotFound => PortableError::MissingEntry(name.into()),
        other => PortableError::Zip(other),
    })?;
    if entry.is_dir() || entry.size() > limit {
        return Err(PortableError::ArchiveTooLarge);
    }
    let capacity = usize::try_from(entry.size()).map_err(|_| PortableError::ArchiveTooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| portable_io("read archive entry", Path::new(name), error))?;
    if portable_count(bytes.len())? > limit {
        return Err(PortableError::ArchiveTooLarge);
    }
    Ok(bytes)
}

fn write_zip_bytes<W: Write + Seek>(
    writer: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), PortableError> {
    writer.start_file(name, options)?;
    writer
        .write_all(bytes)
        .map_err(|error| portable_io("write archive entry", Path::new(name), error))
}

fn unique_ids<'a>(
    values: impl IntoIterator<Item = &'a str>,
    entity: &str,
) -> Result<(), PortableError> {
    let mut seen = HashSet::new();
    for value in values {
        if value.trim().is_empty() || !seen.insert(value) {
            return invalid_collection(&format!("{entity} IDs must be non-empty and unique"));
        }
    }
    Ok(())
}

fn media_entry_path(hash: &str) -> Result<String, PortableError> {
    let digest = canonical_digest(hash)?;
    Ok(format!("media/sha256/{}/{}", &digest[..2], &digest[2..]))
}

fn canonical_digest(hash: &str) -> Result<&str, PortableError> {
    let Some(digest) = hash.strip_prefix(HASH_PREFIX) else {
        return Err(PortableError::InvalidMedia(hash.into()));
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PortableError::InvalidMedia(hash.into()));
    }
    Ok(digest)
}

fn content_hash(bytes: &[u8]) -> String {
    format!("{HASH_PREFIX}{}", hex::encode(Sha256::digest(bytes)))
}

fn hash_file(path: &Path) -> Result<String, PortableError> {
    let mut file = File::open(path).map_err(|error| portable_io("open", path, error))?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| portable_io("read", path, error))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{HASH_PREFIX}{}", hex::encode(digest.finalize())))
}

fn portable_count(value: usize) -> Result<u64, PortableError> {
    u64::try_from(value).map_err(|_| PortableError::ArchiveTooLarge)
}

fn invalid_collection<T>(message: &str) -> Result<T, PortableError> {
    Err(PortableError::InvalidCollection(message.into()))
}

fn portable_io(operation: &'static str, path: &Path, source: io::Error) -> PortableError {
    PortableError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use meiki_domain::{
        Card, Cloze, ComparisonResult, Deck, Direction, Grade, MatchingPolicy, MediaKind,
        MediaReference, MediaRole, OptimizerStatus, ReviewEvent, ReviewEventKind, ScheduleState,
        SchedulerParameterSet, SchedulerProfile, SegmentContent, SemanticSegment, SourceItem,
        StudyIntensity, StudySettingsOverride,
    };
    use tempfile::tempdir;
    use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::{
        ArchiveMediaSource, ArchiveScope, COLLECTION_ENTRY, MANIFEST_ENTRY, PortableCard,
        PortableCollection, PortableError, PortableNote, content_hash, namespace_collection,
        read_archive, write_archive,
    };

    #[test]
    fn multilingual_collection_and_media_round_trip_exactly() {
        let directory = tempdir().unwrap();
        let media_path = directory.path().join("image.bin");
        let media_bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&media_path, media_bytes).unwrap();
        let media_hash = content_hash(media_bytes);
        let collection = collection(&media_hash);
        let archive_path = directory.path().join("collection.meiki");

        let written = write_archive(
            &archive_path,
            &collection,
            &[ArchiveMediaSource {
                content_hash: media_hash.clone(),
                path: media_path,
            }],
            ArchiveScope::FullCollection,
            42,
        )
        .unwrap();
        let restored = read_archive(&archive_path).unwrap();

        assert_eq!(written, restored.manifest);
        assert_eq!(restored.collection, collection);
        assert_eq!(
            restored.collection.notes[0]
                .source_item
                .language_tag
                .as_deref(),
            Some("x-private-meiki-未知")
        );
        let SegmentContent::Text(text) =
            &restored.collection.notes[0].source_item.segments[0].content
        else {
            panic!("expected text segment");
        };
        assert_eq!(text.as_bytes(), "Cafe\u{301} مرحبا 👩🏽‍💻".as_bytes());
        assert_eq!(
            std::fs::read(&restored.media_objects[0].path).unwrap(),
            media_bytes
        );
    }

    #[test]
    fn corrupt_media_and_unexpected_paths_fail_without_extraction() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source.bin");
        let media_bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&source, media_bytes).unwrap();
        let hash = content_hash(media_bytes);
        let collection = collection(&hash);
        let archive = directory.path().join("source.meiki");
        write_archive(
            &archive,
            &collection,
            &[ArchiveMediaSource {
                content_hash: hash,
                path: source,
            }],
            ArchiveScope::FullCollection,
            42,
        )
        .unwrap();

        let corrupt = directory.path().join("corrupt.meiki");
        rewrite_archive(&archive, &corrupt, Some(b"changed"), None);
        assert!(matches!(
            read_archive(&corrupt),
            Err(PortableError::ChecksumMismatch(_) | PortableError::InvalidMedia(_))
        ));

        let traversal = directory.path().join("traversal.meiki");
        rewrite_archive(&archive, &traversal, None, Some(("../outside", b"unsafe")));
        assert!(matches!(
            read_archive(&traversal),
            Err(PortableError::UnexpectedEntry(_))
        ));
        assert!(!directory.path().join("outside").exists());
    }

    #[test]
    fn export_refuses_overwrite_and_merge_namespace_is_stable() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source.bin");
        let bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&source, bytes).unwrap();
        let hash = content_hash(bytes);
        let collection = collection(&hash);
        let destination = directory.path().join("collection.meiki");
        let media = [ArchiveMediaSource {
            content_hash: hash,
            path: source,
        }];
        let manifest = write_archive(
            &destination,
            &collection,
            &media,
            ArchiveScope::FullCollection,
            42,
        )
        .unwrap();
        assert!(matches!(
            write_archive(
                &destination,
                &collection,
                &media,
                ArchiveScope::FullCollection,
                42
            ),
            Err(PortableError::DestinationExists(_))
        ));

        let first = namespace_collection(&collection, &manifest.collection_sha256).unwrap();
        let second = namespace_collection(&collection, &manifest.collection_sha256).unwrap();
        assert_eq!(first, second);
        assert_ne!(first.decks[0].id, collection.decks[0].id);
        assert_eq!(
            first.notes[0].source_item.media[0].content_hash,
            collection.notes[0].source_item.media[0].content_hash
        );
    }

    #[allow(clippy::too_many_lines)]
    fn collection(media_hash: &str) -> PortableCollection {
        let schedule = ScheduleState {
            card_id: "card-1".into(),
            version: 0,
            due_at_ms: 1_000,
            ideal_due_at_ms: 1_000,
            interval_milliseconds: 0,
            interval_seconds: 0,
            repetitions: 0,
            stability_milliseconds: 0,
            difficulty_millipoints: 5_000,
            last_reviewed_at_ms: None,
            last_review_event_id: None,
        };
        let mut reviewed_schedule = schedule.clone();
        reviewed_schedule.version = 1;
        reviewed_schedule.due_at_ms = 86_401_000;
        reviewed_schedule.ideal_due_at_ms = 86_401_000;
        reviewed_schedule.interval_milliseconds = 86_400_000;
        reviewed_schedule.interval_seconds = 86_400;
        reviewed_schedule.repetitions = 1;
        reviewed_schedule.stability_milliseconds = 86_400_000;
        reviewed_schedule.last_reviewed_at_ms = Some(2_000);
        reviewed_schedule.last_review_event_id = Some("review-1".into());
        let event = ReviewEvent {
            id: "review-1".into(),
            card_id: "card-1".into(),
            card_content_version: 1,
            kind: ReviewEventKind::Review,
            undoes_review_event_id: None,
            raw_response: "行きます".into(),
            normalized_response: "行きます".into(),
            comparison: ComparisonResult::Exact,
            suggested_grade: Grade::Good,
            chosen_grade: Grade::Good,
            grade_overridden: false,
            response_duration_ms: 500,
            reviewed_at_ms: 2_000,
            scheduler_version: "fsrs-7".into(),
            scheduler_parameter_set_id: Some("parameters-1".into()),
            target_retention_basis_points: 9_000,
            previous_schedule: schedule.clone(),
            next_schedule: reviewed_schedule.clone(),
        };
        PortableCollection {
            decks: vec![Deck {
                id: "deck-1".into(),
                name: "日本語 العربية".into(),
                description: Some("Cafe\u{301}".into()),
                language_tag: Some("ja".into()),
                direction: Direction::Auto,
                matching_policy: MatchingPolicy::Strict,
                settings: StudySettingsOverride::default(),
                created_at_ms: 1,
                updated_at_ms: 2,
            }],
            notes: vec![PortableNote {
                source_item: SourceItem {
                    id: "note-1".into(),
                    deck_id: "deck-1".into(),
                    segments: vec![
                        SemanticSegment {
                            id: "segment-1".into(),
                            ordinal: 0,
                            content: SegmentContent::Text("Cafe\u{301} مرحبا 👩🏽‍💻".into()),
                        },
                        SemanticSegment {
                            id: "segment-2".into(),
                            ordinal: 1,
                            content: SegmentContent::Cloze {
                                cloze_id: "cloze-1".into(),
                                text: "行きます".into(),
                            },
                        },
                    ],
                    language_tag: Some("x-private-meiki-未知".into()),
                    direction: Direction::Auto,
                    tags: Vec::new(),
                    annotations: Vec::new(),
                    explanation: None,
                    media: vec![MediaReference {
                        id: "media-1".into(),
                        content_hash: media_hash.into(),
                        kind: MediaKind::Image,
                        role: MediaRole::RevealImage,
                        media_type: "image/png".into(),
                        byte_size: 22,
                        original_file_name: Some("../../危険.png".into()),
                        alt_text: Some("画像".into()),
                        width: None,
                        height: None,
                        duration_ms: None,
                        language_tag: Some("ja".into()),
                        direction: Direction::Auto,
                        created_at_ms: 1,
                    }],
                    created_at_ms: 1,
                    updated_at_ms: 2,
                },
                clozes: vec![Cloze {
                    id: "cloze-1".into(),
                    source_item_id: "note-1".into(),
                    answer: "行きます".into(),
                    accepted_answers: vec!["ゆきます".into()],
                    hint: None,
                    language_tag: Some("ja".into()),
                    direction: Direction::Auto,
                    matching_policy: None,
                    annotations: Vec::new(),
                    explanation: None,
                    media: Vec::new(),
                    created_at_ms: 1,
                    updated_at_ms: 2,
                }],
                cards: vec![PortableCard {
                    card: Card {
                        id: "card-1".into(),
                        cloze_id: "cloze-1".into(),
                        content_version: 1,
                        suspended: false,
                        settings: StudySettingsOverride::default(),
                        created_at_ms: 1,
                        updated_at_ms: 2,
                    },
                    baseline: schedule.clone(),
                    schedule: reviewed_schedule,
                    review_events: vec![event],
                }],
                deleted_at_ms: None,
            }],
            scheduler_parameter_sets: vec![SchedulerParameterSet {
                id: "parameters-1".into(),
                engine_version: "fsrs-7".into(),
                parameters: vec![0.1, 0.2],
                created_at_ms: 1,
            }],
            scheduler_profiles: vec![SchedulerProfile {
                deck_id: "deck-1".into(),
                engine_version: "fsrs-7".into(),
                active_parameter_set_id: "parameters-1".into(),
                previous_parameter_set_id: None,
                intensity: StudyIntensity::Balanced,
                daily_time_budget_minutes: None,
                day_boundary_minutes: 240,
                optimizer_status: OptimizerStatus::NeverRun,
                optimizer_diagnostics: None,
                updated_at_ms: 2,
            }],
        }
    }

    fn rewrite_archive(
        source: &std::path::Path,
        destination: &std::path::Path,
        corrupt_media: Option<&[u8]>,
        extra: Option<(&str, &[u8])>,
    ) {
        let mut input = ZipArchive::new(std::fs::File::open(source).unwrap()).unwrap();
        let output = std::fs::File::create(destination).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..input.len() {
            let mut entry = input.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            writer.start_file(&name, options).unwrap();
            if let Some(corrupt) = corrupt_media
                && name != MANIFEST_ENTRY
                && name != COLLECTION_ENTRY
            {
                writer.write_all(corrupt).unwrap();
            } else {
                writer.write_all(&bytes).unwrap();
            }
        }
        if let Some((name, bytes)) = extra {
            writer.start_file(name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
}
