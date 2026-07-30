//! Versioned, validated `.meiki` collection archives.
//!
//! The archive stores canonical UTF-8 JSON and checksum-addressed media. It
//! never stores or extracts caller-controlled filesystem paths.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use meiki_domain::{
    Card, CardLifecycle, Cloze, CollectionSchedulingSettings, Deck, ReviewEvent, ReviewEventKind,
    ScheduleState, SchedulerParameterSet, SchedulerProfile, SourceItem,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::{NamedTempFile, TempDir};
use thiserror::Error;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const ARCHIVE_FORMAT: &str = "meiki";
pub const ARCHIVE_VERSION: u32 = 4;
const POLICY_ARCHIVE_VERSION: u32 = 3;
const LEGACY_ARCHIVE_VERSION: u32 = 1;
const MANIFEST_ENTRY: &str = "manifest.json";
const COLLECTION_ENTRY: &str = "collection.json";
const MAX_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_UNCOMPRESSED_BYTES: u64 = MAX_ARCHIVE_BYTES;
const MAX_COLLECTION_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MEDIA_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_002;
const MAX_COMPRESSION_RATIO: u64 = 1_000;
const COMPRESSION_RATIO_MINIMUM_BYTES: u64 = 1024 * 1024;
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
    #[serde(default)]
    pub collection_scheduling_settings: CollectionSchedulingSettings,
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
    #[error("archive entry has a suspicious compression ratio: {0}")]
    SuspiciousCompression(String),
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
        scope: ArchiveScope::FullCollection,
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
    let names = archive_names(path, archive.central_directory_start())?;
    if names.len() != archive.len() {
        return Err(PortableError::UnexpectedEntry("duplicate entry".into()));
    }
    preflight_archive_entries(&mut archive)?;
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
    let mut collection: PortableCollection = serde_json::from_slice(&collection_bytes)?;
    if manifest.version < POLICY_ARCHIVE_VERSION {
        upgrade_legacy_policy(&mut collection);
    }
    if manifest.version == LEGACY_ARCHIVE_VERSION {
        restore_legacy_lifecycles(&mut collection)?;
    }
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

fn upgrade_legacy_policy(collection: &mut PortableCollection) {
    let promoted_budget = collection
        .scheduler_profiles
        .iter()
        .find(|profile| profile.deck_id == "default-deck")
        .and_then(|profile| profile.deck_daily_time_budget_minutes)
        .or_else(|| {
            collection
                .scheduler_profiles
                .iter()
                .find_map(|profile| profile.deck_daily_time_budget_minutes)
        })
        .or_else(|| {
            collection
                .scheduler_profiles
                .first()
                .map(|profile| match profile.legacy_intensity {
                    meiki_domain::LegacyStudyIntensity::Light => 15,
                    meiki_domain::LegacyStudyIntensity::Balanced => 30,
                    meiki_domain::LegacyStudyIntensity::Intensive => 60,
                })
        })
        .unwrap_or(30);
    collection
        .collection_scheduling_settings
        .daily_time_budget_minutes = promoted_budget.clamp(1, 1_440);

    let decks = collection
        .decks
        .iter()
        .map(|deck| (deck.id.as_str(), &deck.settings))
        .collect::<HashMap<_, _>>();
    for profile in &mut collection.scheduler_profiles {
        let settings = decks.get(profile.deck_id.as_str()).copied();
        let has_manual_policy = settings.is_some_and(|settings| {
            settings.target_retention_basis_points.is_some()
                || settings.new_cards_per_day.is_some()
                || settings.maximum_interval_days.is_some()
        });
        profile.scheduling_mode = if has_manual_policy {
            meiki_domain::SchedulingMode::Expert
        } else {
            meiki_domain::SchedulingMode::Automatic
        };
        profile.controller_target_retention_basis_points = settings
            .and_then(|settings| settings.target_retention_basis_points)
            .unwrap_or(match profile.legacy_intensity {
                meiki_domain::LegacyStudyIntensity::Light => 8_500,
                meiki_domain::LegacyStudyIntensity::Balanced => 9_000,
                meiki_domain::LegacyStudyIntensity::Intensive => 9_300,
            })
            .clamp(8_000, 9_500);
        profile.controller_new_cards_per_day = settings
            .and_then(|settings| settings.new_cards_per_day)
            .unwrap_or(20)
            .min(10_000);
        profile.controller_explanation =
            "Migrated policy settings; automatic mode evaluates on the next Today view.".into();
        if profile.deck_id == "default-deck" {
            profile.deck_daily_time_budget_minutes = None;
        }
    }
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
    if !(1..=1_440).contains(
        &collection
            .collection_scheduling_settings
            .daily_time_budget_minutes,
    ) {
        return invalid_collection("the collection daily budget is outside safe bounds");
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
        if profile.engine_version.is_empty()
            || profile.controller_version.is_empty()
            || !(8_000..=9_500).contains(&profile.controller_target_retention_basis_points)
            || profile.controller_new_cards_per_day > 10_000
            || profile
                .deck_daily_time_budget_minutes
                .is_some_and(|budget| !(1..=1_440).contains(&budget))
            || profile.day_boundary_minutes >= 1_440
        {
            return invalid_collection("scheduler profile controls are outside safe bounds");
        }
        if !deck_ids.contains(profile.deck_id.as_str())
            || !parameter_ids.contains(profile.active_parameter_set_id.as_str())
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
            if let meiki_domain::SegmentContent::Cloze { cloze_id, .. } = &segment.content {
                if !cloze_ids.contains(cloze_id.as_str()) {
                    return invalid_collection("segment references a missing cloze");
                }
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
            if let Some(existing) = tags.insert(tag.id.as_str(), tag) {
                if existing != tag {
                    return invalid_collection("a reused tag ID has conflicting content");
                }
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
            if let Some(existing) = media_references.insert(media.id.as_str(), media) {
                if existing != media {
                    return invalid_collection(
                        "a reused media reference ID has conflicting metadata",
                    );
                }
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
    let mut active_reviews = Vec::<(&str, &ScheduleState)>::new();
    for event in &portable.review_events {
        if event.id.trim().is_empty()
            || event.card_id != card_id
            || event.previous_schedule != projected
            || event.next_schedule.card_id != card_id
            || event.previous_schedule.version.checked_add(1) != Some(event.next_schedule.version)
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
                active_reviews.push((event.id.as_str(), &event.previous_schedule));
            }
            ReviewEventKind::Undo => {
                let Some(target) = event.undoes_review_event_id.as_deref() else {
                    return invalid_collection("undo event has no review target");
                };
                let Some((latest_id, prior_schedule)) = active_reviews.last() else {
                    return invalid_collection("undo event has no active review target");
                };
                if *latest_id != target {
                    return invalid_collection("undo event target is not the latest active review");
                }
                let mut restored = (*prior_schedule).clone();
                restored.version = event.next_schedule.version;
                restored.last_review_event_id = Some(event.id.clone());
                if event.next_schedule != restored {
                    return invalid_collection(
                        "undo event does not restore the compensated review snapshot",
                    );
                }
                active_reviews.pop();
            }
            ReviewEventKind::Review => {
                return invalid_collection("review event cannot undo another review");
            }
        }
        let expected_lifecycle = if active_reviews.is_empty() {
            portable.baseline.lifecycle
        } else {
            CardLifecycle::Introduced
        };
        if event.next_schedule.lifecycle != expected_lifecycle {
            return invalid_collection("review history lifecycle transition is invalid");
        }
        projected = event.next_schedule.clone();
    }
    if projected != portable.schedule {
        return invalid_collection("current schedule does not match review history");
    }
    Ok(())
}

fn restore_legacy_lifecycles(collection: &mut PortableCollection) -> Result<(), PortableError> {
    for portable in collection
        .notes
        .iter_mut()
        .flat_map(|note| note.cards.iter_mut())
    {
        portable.baseline.lifecycle = lifecycle_from_memory(&portable.baseline);
        let baseline_lifecycle = portable.baseline.lifecycle;
        let mut active_reviews = Vec::<String>::new();
        for event in &mut portable.review_events {
            event.previous_schedule.lifecycle = if active_reviews.is_empty() {
                baseline_lifecycle
            } else {
                CardLifecycle::Introduced
            };
            match event.kind {
                ReviewEventKind::Review if event.undoes_review_event_id.is_none() => {
                    active_reviews.push(event.id.clone());
                }
                ReviewEventKind::Undo => {
                    let Some(target) = event.undoes_review_event_id.as_deref() else {
                        return invalid_collection("undo event has no review target");
                    };
                    if active_reviews.last().map(String::as_str) != Some(target) {
                        return invalid_collection(
                            "undo event target is not the latest active review",
                        );
                    }
                    active_reviews.pop();
                }
                ReviewEventKind::Review => {
                    return invalid_collection("review event cannot undo another review");
                }
            }
            event.next_schedule.lifecycle = if active_reviews.is_empty() {
                baseline_lifecycle
            } else {
                CardLifecycle::Introduced
            };
        }
        portable.schedule.lifecycle = if active_reviews.is_empty() {
            lifecycle_from_memory(&portable.schedule)
        } else {
            CardLifecycle::Introduced
        };
    }
    Ok(())
}

const fn lifecycle_from_memory(schedule: &ScheduleState) -> CardLifecycle {
    if schedule.repetitions > 0
        || schedule.stability_milliseconds > 0
        || schedule.difficulty_millipoints > 0
        || schedule.last_reviewed_at_ms.is_some()
    {
        CardLifecycle::Introduced
    } else {
        CardLifecycle::Unseen
    }
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
        if let Some(existing) = technical_metadata.insert(media.content_hash.as_str(), metadata) {
            if existing != metadata {
                return Err(PortableError::InvalidMedia(format!(
                    "{} has conflicting technical metadata",
                    media.content_hash
                )));
            }
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
        || !(LEGACY_ARCHIVE_VERSION..=ARCHIVE_VERSION).contains(&manifest.version)
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

fn archive_names(
    path: &Path,
    central_directory_start: u64,
) -> Result<HashSet<String>, PortableError> {
    const CENTRAL_HEADER_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
    const END_SIGNATURES: [[u8; 4]; 2] = [[0x50, 0x4b, 0x05, 0x06], [0x50, 0x4b, 0x06, 0x06]];

    let mut file = File::open(path).map_err(|error| portable_io("open", path, error))?;
    file.seek(SeekFrom::Start(central_directory_start))
        .map_err(|error| portable_io("seek central directory", path, error))?;
    let mut names = HashSet::new();
    loop {
        let mut signature = [0_u8; 4];
        file.read_exact(&mut signature)
            .map_err(|error| portable_io("read central directory", path, error))?;
        if END_SIGNATURES.contains(&signature) {
            break;
        }
        if signature != CENTRAL_HEADER_SIGNATURE {
            return Err(PortableError::UnexpectedEntry(
                "<invalid central directory>".into(),
            ));
        }

        let mut fixed = [0_u8; 42];
        file.read_exact(&mut fixed)
            .map_err(|error| portable_io("read central directory", path, error))?;
        let name_length = usize::from(u16::from_le_bytes([fixed[24], fixed[25]]));
        let extra_length = u64::from(u16::from_le_bytes([fixed[26], fixed[27]]));
        let comment_length = u64::from(u16::from_le_bytes([fixed[28], fixed[29]]));
        let mut raw_name = vec![0_u8; name_length];
        file.read_exact(&mut raw_name)
            .map_err(|error| portable_io("read archive entry name", path, error))?;
        let name = String::from_utf8(raw_name)
            .map_err(|_| PortableError::UnexpectedEntry("<non-UTF-8 entry name>".into()))?;
        if name.ends_with('/') || !names.insert(name.clone()) {
            return Err(PortableError::UnexpectedEntry(name));
        }
        if names.len() > MAX_ENTRIES {
            return Err(PortableError::TooManyEntries);
        }
        let skip = extra_length
            .checked_add(comment_length)
            .ok_or(PortableError::ArchiveTooLarge)?;
        file.seek(SeekFrom::Current(
            i64::try_from(skip).map_err(|_| PortableError::ArchiveTooLarge)?,
        ))
        .map_err(|error| portable_io("seek central directory", path, error))?;
    }
    Ok(names)
}

fn preflight_archive_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<(), PortableError> {
    let mut total_uncompressed = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        validate_entry_budget(
            entry.name(),
            entry.size(),
            entry.compressed_size(),
            &mut total_uncompressed,
        )?;
    }
    Ok(())
}

fn validate_entry_budget(
    name: &str,
    uncompressed_size: u64,
    compressed_size: u64,
    total_uncompressed: &mut u64,
) -> Result<(), PortableError> {
    *total_uncompressed = total_uncompressed
        .checked_add(uncompressed_size)
        .ok_or(PortableError::ArchiveTooLarge)?;
    if *total_uncompressed > MAX_ARCHIVE_UNCOMPRESSED_BYTES {
        return Err(PortableError::ArchiveTooLarge);
    }
    if uncompressed_size >= COMPRESSION_RATIO_MINIMUM_BYTES
        && (compressed_size == 0
            || compressed_size
                .checked_mul(MAX_COMPRESSION_RATIO)
                .is_none_or(|maximum| uncompressed_size > maximum))
    {
        return Err(PortableError::SuspiciousCompression(name.to_owned()));
    }
    Ok(())
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
        Card, CardLifecycle, Cloze, CollectionSchedulingSettings, ComparisonResult, Deck,
        Direction, Grade, LegacyStudyIntensity, MatchingPolicy, MediaKind, MediaReference,
        MediaRole, ReviewEvent, ReviewEventKind, ScheduleState, SchedulerParameterSet,
        SchedulerProfile, SchedulingMode, SegmentContent, SemanticSegment, SourceItem,
        StudySettingsOverride,
    };
    use tempfile::tempdir;
    use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::{
        ArchiveManifest, ArchiveMediaSource, ArchiveScope, COLLECTION_ENTRY,
        COMPRESSION_RATIO_MINIMUM_BYTES, MANIFEST_ENTRY, MAX_ARCHIVE_UNCOMPRESSED_BYTES,
        MAX_COMPRESSION_RATIO, PortableCard, PortableCollection, PortableError, PortableNote,
        content_hash, read_archive, validate_collection, validate_entry_budget, write_archive,
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
            42,
        )
        .unwrap();
        let restored = read_archive(&archive_path).unwrap();

        assert_eq!(written, restored.manifest);
        assert_eq!(written.version, 4);
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
    fn version_one_archive_derives_lifecycle_from_immutable_history() {
        let directory = tempdir().unwrap();
        let media_path = directory.path().join("image.bin");
        let media_bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&media_path, media_bytes).unwrap();
        let media_hash = content_hash(media_bytes);
        let collection = collection(&media_hash);
        let current = directory.path().join("current.meiki");
        write_archive(
            &current,
            &collection,
            &[ArchiveMediaSource {
                content_hash: media_hash,
                path: media_path,
            }],
            42,
        )
        .unwrap();
        let legacy = directory.path().join("version-1.meiki");
        rewrite_as_version_one(&current, &legacy);

        let restored = read_archive(&legacy).unwrap();

        assert_eq!(restored.manifest.version, 1);
        let mut expected = collection;
        expected.scheduler_profiles[0].controller_explanation =
            "Migrated policy settings; automatic mode evaluates on the next Today view.".into();
        assert_eq!(restored.collection, expected);
        assert_eq!(
            restored.collection.notes[0].cards[0].schedule.lifecycle,
            CardLifecycle::Introduced
        );
    }

    #[test]
    fn version_two_archive_migrates_manual_policy_and_budget() {
        let directory = tempdir().unwrap();
        let media_path = directory.path().join("image.bin");
        let media_bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&media_path, media_bytes).unwrap();
        let media_hash = content_hash(media_bytes);
        let mut source_collection = collection(&media_hash);
        source_collection.decks[0]
            .settings
            .target_retention_basis_points = Some(9_250);
        source_collection.scheduler_profiles[0].deck_daily_time_budget_minutes = Some(45);
        let current = directory.path().join("current.meiki");
        write_archive(
            &current,
            &source_collection,
            &[ArchiveMediaSource {
                content_hash: media_hash,
                path: media_path,
            }],
            42,
        )
        .unwrap();
        let legacy = directory.path().join("version-2.meiki");
        rewrite_as_version_two(&current, &legacy);

        let restored = read_archive(&legacy).unwrap();

        assert_eq!(restored.manifest.version, 2);
        assert_eq!(
            restored
                .collection
                .collection_scheduling_settings
                .daily_time_budget_minutes,
            45
        );
        assert_eq!(
            restored.collection.scheduler_profiles[0].scheduling_mode,
            SchedulingMode::Expert
        );
        assert_eq!(
            restored.collection.scheduler_profiles[0].controller_target_retention_basis_points,
            9_250
        );
    }

    #[test]
    fn version_three_archive_with_removed_override_fields_remains_readable() {
        let directory = tempdir().unwrap();
        let media_path = directory.path().join("image.bin");
        let media_bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&media_path, media_bytes).unwrap();
        let media_hash = content_hash(media_bytes);
        let collection = collection(&media_hash);
        let current = directory.path().join("current.meiki");
        write_archive(
            &current,
            &collection,
            &[ArchiveMediaSource {
                content_hash: media_hash,
                path: media_path,
            }],
            42,
        )
        .unwrap();
        let legacy = directory.path().join("version-3.meiki");
        rewrite_as_version_three(&current, &legacy);

        let restored = read_archive(&legacy).unwrap();

        assert_eq!(restored.manifest.version, 3);
        assert_eq!(restored.collection, collection);
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
    fn archive_hostility_rejects_absolute_duplicate_partial_and_malformed_entries() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source.bin");
        let media_bytes = b"\x89PNG\r\n\x1a\nportable-media";
        std::fs::write(&source, media_bytes).unwrap();
        let hash = content_hash(media_bytes);
        let archive = directory.path().join("source.meiki");
        write_archive(
            &archive,
            &collection(&hash),
            &[ArchiveMediaSource {
                content_hash: hash,
                path: source,
            }],
            42,
        )
        .unwrap();

        for (name, archive_name) in [
            ("/absolute-outside", "absolute.meiki"),
            ("C:\\absolute-outside", "windows-absolute.meiki"),
        ] {
            let hostile = directory.path().join(archive_name);
            rewrite_archive(&archive, &hostile, None, Some((name, b"unsafe")));
            assert!(matches!(
                read_archive(&hostile),
                Err(PortableError::UnexpectedEntry(_))
            ));
        }

        let duplicate = directory.path().join("duplicate.meiki");
        rewrite_archive(&archive, &duplicate, None, Some(("manifesx.json", b"{}")));
        rewrite_zip_name(&duplicate, b"manifesx.json", MANIFEST_ENTRY.as_bytes());
        let duplicate_result = read_archive(&duplicate);
        assert!(
            matches!(duplicate_result, Err(PortableError::UnexpectedEntry(_))),
            "{duplicate_result:?}"
        );

        let invalid_name = directory.path().join("invalid-name.meiki");
        rewrite_archive(
            &archive,
            &invalid_name,
            None,
            Some(("invalid-name", b"unsafe")),
        );
        rewrite_zip_name(&invalid_name, b"invalid-name", b"\xffnvalid-name");
        assert!(matches!(
            read_archive(&invalid_name),
            Err(PortableError::UnexpectedEntry(_))
        ));

        let malformed_manifest = directory.path().join("malformed-manifest.meiki");
        rewrite_named_entry(
            &archive,
            &malformed_manifest,
            MANIFEST_ENTRY,
            b"\xffnot-utf8-json",
        );
        assert!(matches!(
            read_archive(&malformed_manifest),
            Err(PortableError::Json(_))
        ));

        let malformed_collection = directory.path().join("malformed-collection.meiki");
        rewrite_named_entry(
            &archive,
            &malformed_collection,
            COLLECTION_ENTRY,
            b"not-json",
        );
        assert!(matches!(
            read_archive(&malformed_collection),
            Err(PortableError::ChecksumMismatch(_))
        ));

        let partial = directory.path().join("partial.meiki");
        let archive_bytes = std::fs::read(&archive).unwrap();
        std::fs::write(&partial, &archive_bytes[..archive_bytes.len() / 2]).unwrap();
        assert!(matches!(
            read_archive(&partial),
            Err(PortableError::Zip(_) | PortableError::Io { .. })
        ));
        assert!(!directory.path().join("absolute-outside").exists());
    }

    #[test]
    fn aggregate_and_compression_budgets_reject_zip_bomb_shapes() {
        let mut total = 0;
        validate_entry_budget(
            "maximum",
            MAX_ARCHIVE_UNCOMPRESSED_BYTES,
            MAX_ARCHIVE_UNCOMPRESSED_BYTES,
            &mut total,
        )
        .unwrap();
        assert!(matches!(
            validate_entry_budget("one-byte-too-many", 1, 1, &mut total),
            Err(PortableError::ArchiveTooLarge)
        ));

        let mut total = 0;
        let compressed = COMPRESSION_RATIO_MINIMUM_BYTES / MAX_COMPRESSION_RATIO;
        assert!(matches!(
            validate_entry_budget(
                "compressed-bomb",
                COMPRESSION_RATIO_MINIMUM_BYTES,
                compressed,
                &mut total,
            ),
            Err(PortableError::SuspiciousCompression(_))
        ));

        let directory = tempdir().unwrap();
        let compressed_bomb = directory.path().join("compressed-bomb.meiki");
        let output = std::fs::File::create(&compressed_bomb).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer.start_file(MANIFEST_ENTRY, options).unwrap();
        writer.write_all(&vec![0_u8; 2 * 1024 * 1024]).unwrap();
        writer.finish().unwrap();
        assert!(matches!(
            read_archive(&compressed_bomb),
            Err(PortableError::SuspiciousCompression(_))
        ));
    }

    #[test]
    fn export_refuses_overwrite_and_writes_full_collection_scope() {
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
        let manifest = write_archive(&destination, &collection, &media, 42).unwrap();
        assert_eq!(manifest.scope, ArchiveScope::FullCollection);
        assert!(matches!(
            write_archive(&destination, &collection, &media, 42),
            Err(PortableError::DestinationExists(_))
        ));
    }

    #[test]
    fn archive_validation_rejects_discontinuous_duplicated_reordered_and_malformed_history() {
        let media_hash = content_hash(b"portable-history");
        let valid = collection(&media_hash);

        let mut discontinuous = valid.clone();
        discontinuous.notes[0].cards[0].review_events[0]
            .previous_schedule
            .version = 7;
        assert!(validate_collection(&discontinuous).is_err());

        let mut duplicated = valid.clone();
        let duplicated_event = duplicated.notes[0].cards[0].review_events[0].clone();
        duplicated.notes[0].cards[0]
            .review_events
            .push(duplicated_event);
        assert!(validate_collection(&duplicated).is_err());

        let mut ordered = valid.clone();
        let first = ordered.notes[0].cards[0].review_events[0].clone();
        let mut second_schedule = first.next_schedule.clone();
        second_schedule.version += 1;
        second_schedule.due_at_ms += 86_400_000;
        second_schedule.ideal_due_at_ms += 86_400_000;
        second_schedule.last_review_event_id = Some("review-2".into());
        let mut second = first;
        second.id = "review-2".into();
        second.reviewed_at_ms += 10_000;
        second.previous_schedule = second.next_schedule;
        second.next_schedule = second_schedule.clone();
        ordered.notes[0].cards[0].schedule = second_schedule;
        ordered.notes[0].cards[0].review_events.push(second);
        assert!(validate_collection(&ordered).is_ok());
        ordered.notes[0].cards[0].review_events.reverse();
        assert!(validate_collection(&ordered).is_err());

        let mut malformed = valid;
        malformed.notes[0].cards[0].review_events[0].kind = ReviewEventKind::Undo;
        malformed.notes[0].cards[0].review_events[0].undoes_review_event_id =
            Some("missing-review".into());
        assert!(validate_collection(&malformed).is_err());
    }

    #[allow(clippy::too_many_lines)]
    fn collection(media_hash: &str) -> PortableCollection {
        let schedule = ScheduleState {
            card_id: "card-1".into(),
            version: 0,
            lifecycle: CardLifecycle::Unseen,
            due_at_ms: 1_000,
            ideal_due_at_ms: 1_000,
            interval_milliseconds: 0,
            interval_seconds: 0,
            repetitions: 0,
            stability_milliseconds: 0,
            difficulty_millipoints: 0,
            last_reviewed_at_ms: None,
            last_review_event_id: None,
        };
        let mut reviewed_schedule = schedule.clone();
        reviewed_schedule.version = 1;
        reviewed_schedule.lifecycle = CardLifecycle::Introduced;
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
            collection_scheduling_settings: CollectionSchedulingSettings::default(),
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
                scheduling_mode: SchedulingMode::Automatic,
                deck_daily_time_budget_minutes: None,
                controller_version: "time-budget-v1".into(),
                controller_target_retention_basis_points: 9_000,
                controller_new_cards_per_day: 20,
                controller_last_evaluated_day_start_ms: None,
                controller_review_count: 0,
                controller_unseen_count: 0,
                controller_forecast_review_seconds_per_day: 0,
                controller_backlog_exceeds_budget: false,
                controller_explanation: String::new(),
                legacy_intensity: LegacyStudyIntensity::Balanced,
                day_boundary_minutes: 240,
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
            if name != MANIFEST_ENTRY && name != COLLECTION_ENTRY {
                if let Some(corrupt) = corrupt_media {
                    writer.write_all(corrupt).unwrap();
                    continue;
                }
            }
            writer.write_all(&bytes).unwrap();
        }
        if let Some((name, bytes)) = extra {
            writer.start_file(name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn rewrite_named_entry(
        source: &std::path::Path,
        destination: &std::path::Path,
        target: &str,
        replacement: &[u8],
    ) {
        let mut input = ZipArchive::new(std::fs::File::open(source).unwrap()).unwrap();
        let output = std::fs::File::create(destination).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..input.len() {
            let mut entry = input.by_index(index).unwrap();
            let name = entry.name().to_owned();
            writer.start_file(&name, options).unwrap();
            if name == target {
                writer.write_all(replacement).unwrap();
            } else {
                std::io::copy(&mut entry, &mut writer).unwrap();
            }
        }
        writer.finish().unwrap();
    }

    fn rewrite_zip_name(path: &std::path::Path, old: &[u8], new: &[u8]) {
        assert_eq!(old.len(), new.len());
        let mut bytes = std::fs::read(path).unwrap();
        let mut replacements = 0;
        for index in 0..=bytes.len() - old.len() {
            if bytes[index..index + old.len()] == *old {
                bytes[index..index + new.len()].copy_from_slice(new);
                replacements += 1;
            }
        }
        assert_eq!(replacements, 2, "local and central names must be replaced");
        std::fs::write(path, bytes).unwrap();
    }

    fn rewrite_as_version_one(source: &std::path::Path, destination: &std::path::Path) {
        let mut input = ZipArchive::new(std::fs::File::open(source).unwrap()).unwrap();
        let mut entries = Vec::new();
        for index in 0..input.len() {
            let mut entry = input.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            entries.push((name, bytes));
        }

        let collection_entry = entries
            .iter_mut()
            .find(|(name, _)| name == COLLECTION_ENTRY)
            .unwrap();
        let mut collection_json: serde_json::Value =
            serde_json::from_slice(&collection_entry.1).unwrap();
        remove_lifecycle_fields(&mut collection_json);
        collection_entry.1 = serde_json::to_vec(&collection_json).unwrap();
        let collection_sha256 = content_hash(&collection_entry.1);

        let manifest_entry = entries
            .iter_mut()
            .find(|(name, _)| name == MANIFEST_ENTRY)
            .unwrap();
        let mut manifest: ArchiveManifest = serde_json::from_slice(&manifest_entry.1).unwrap();
        manifest.version = 1;
        manifest.collection_sha256 = collection_sha256;
        manifest_entry.1 = serde_json::to_vec(&manifest).unwrap();

        let output = std::fs::File::create(destination).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn rewrite_as_version_two(source: &std::path::Path, destination: &std::path::Path) {
        let mut input = ZipArchive::new(std::fs::File::open(source).unwrap()).unwrap();
        let mut entries = Vec::new();
        for index in 0..input.len() {
            let mut entry = input.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            entries.push((name, bytes));
        }

        let collection_entry = entries
            .iter_mut()
            .find(|(name, _)| name == COLLECTION_ENTRY)
            .unwrap();
        let mut collection_json: serde_json::Value =
            serde_json::from_slice(&collection_entry.1).unwrap();
        let root = collection_json.as_object_mut().unwrap();
        root.remove("collection_scheduling_settings");
        for profile in root
            .get_mut("scheduler_profiles")
            .unwrap()
            .as_array_mut()
            .unwrap()
        {
            let profile = profile.as_object_mut().unwrap();
            let budget = profile.remove("deck_daily_time_budget_minutes").unwrap();
            profile.insert("daily_time_budget_minutes".into(), budget);
            for field in [
                "scheduling_mode",
                "controller_version",
                "controller_target_retention_basis_points",
                "controller_new_cards_per_day",
                "controller_last_evaluated_day_start_ms",
                "controller_review_count",
                "controller_unseen_count",
                "controller_forecast_review_seconds_per_day",
                "controller_backlog_exceeds_budget",
                "controller_explanation",
            ] {
                profile.remove(field);
            }
        }
        collection_entry.1 = serde_json::to_vec(&collection_json).unwrap();
        let collection_sha256 = content_hash(&collection_entry.1);

        let manifest_entry = entries
            .iter_mut()
            .find(|(name, _)| name == MANIFEST_ENTRY)
            .unwrap();
        let mut manifest: ArchiveManifest = serde_json::from_slice(&manifest_entry.1).unwrap();
        manifest.version = 2;
        manifest.collection_sha256 = collection_sha256;
        manifest_entry.1 = serde_json::to_vec(&manifest).unwrap();

        let output = std::fs::File::create(destination).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn rewrite_as_version_three(source: &std::path::Path, destination: &std::path::Path) {
        let mut input = ZipArchive::new(std::fs::File::open(source).unwrap()).unwrap();
        let mut entries = Vec::new();
        for index in 0..input.len() {
            let mut entry = input.by_index(index).unwrap();
            let name = entry.name().to_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            entries.push((name, bytes));
        }

        let collection_entry = entries
            .iter_mut()
            .find(|(name, _)| name == COLLECTION_ENTRY)
            .unwrap();
        let mut collection_json: serde_json::Value =
            serde_json::from_slice(&collection_entry.1).unwrap();
        let root = collection_json.as_object_mut().unwrap();
        for note in root.get_mut("notes").unwrap().as_array_mut().unwrap() {
            for portable_card in note.get_mut("cards").unwrap().as_array_mut().unwrap() {
                portable_card
                    .get_mut("card")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert(
                        "settings".into(),
                        serde_json::json!({
                            "target_retention_basis_points": null,
                            "new_cards_per_day": null,
                            "maximum_interval_days": null
                        }),
                    );
            }
        }
        for profile in root
            .get_mut("scheduler_profiles")
            .unwrap()
            .as_array_mut()
            .unwrap()
        {
            let profile = profile.as_object_mut().unwrap();
            profile.insert("previous_parameter_set_id".into(), serde_json::Value::Null);
            profile.insert("intensity".into(), serde_json::json!("balanced"));
            profile.insert("optimizer_status".into(), serde_json::json!("never_run"));
            profile.insert("optimizer_diagnostics".into(), serde_json::Value::Null);
        }
        collection_entry.1 = serde_json::to_vec(&collection_json).unwrap();
        let collection_sha256 = content_hash(&collection_entry.1);

        let manifest_entry = entries
            .iter_mut()
            .find(|(name, _)| name == MANIFEST_ENTRY)
            .unwrap();
        let mut manifest: ArchiveManifest = serde_json::from_slice(&manifest_entry.1).unwrap();
        manifest.version = 3;
        manifest.collection_sha256 = collection_sha256;
        manifest_entry.1 = serde_json::to_vec(&manifest).unwrap();

        let output = std::fs::File::create(destination).unwrap();
        let mut writer = ZipWriter::new(output);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    fn remove_lifecycle_fields(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(values) => {
                values.remove("lifecycle");
                for nested in values.values_mut() {
                    remove_lifecycle_fields(nested);
                }
            }
            serde_json::Value::Array(values) => {
                for nested in values {
                    remove_lifecycle_fields(nested);
                }
            }
            _ => {}
        }
    }
}
