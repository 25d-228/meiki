use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use tauri::http::{
    Method, Request, Response, StatusCode,
    header::{
        ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, ALLOW,
        CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
    },
};

pub(crate) const PROTOCOL: &str = "meiki-media";

const AUDIO_MPEG: &str = "audio/mpeg";
const BYTE_RANGE_UNIT: &str = "bytes";
const SHA256_DIGEST_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteRange {
    start: u64,
    end: u64,
}

pub(crate) fn response(collection_path: &Path, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(ALLOW, "GET, HEAD")
            .body(Vec::new())
            .expect("static managed-media response headers are valid");
    }

    let Some(digest) = request_digest(request.uri().path()) else {
        return empty_response(StatusCode::NOT_FOUND);
    };
    let object_path = collection_path
        .with_extension("media")
        .join("objects")
        .join("sha256")
        .join(&digest[..2])
        .join(&digest[2..]);
    let Ok(mut object) = File::open(object_path) else {
        return empty_response(StatusCode::NOT_FOUND);
    };
    let Ok(object_length) = object.metadata().map(|metadata| metadata.len()) else {
        return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let requested_range = match request.headers().get(RANGE) {
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| parse_range(value, object_length))
        {
            Some(range) => Some(range),
            None => return range_not_satisfiable(object_length),
        },
        None => None,
    };
    let (status, content_length, content_range) =
        requested_range.map_or((StatusCode::OK, object_length, None), |range| {
            (
                StatusCode::PARTIAL_CONTENT,
                range.end - range.start + 1,
                Some(format!(
                    "{BYTE_RANGE_UNIT} {}-{}/{object_length}",
                    range.start, range.end
                )),
            )
        });

    let body = if request.method() == Method::HEAD {
        Vec::new()
    } else {
        match read_body(&mut object, requested_range, content_length) {
            Ok(body) => body,
            Err(()) => return empty_response(StatusCode::INTERNAL_SERVER_ERROR),
        }
    };

    let mut response = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, AUDIO_MPEG)
        .header(ACCEPT_RANGES, BYTE_RANGE_UNIT)
        .header(CONTENT_LENGTH, content_length)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            ACCESS_CONTROL_EXPOSE_HEADERS,
            "accept-ranges, content-length, content-range",
        );
    if let Some(content_range) = content_range {
        response = response.header(CONTENT_RANGE, content_range);
    }
    response
        .body(body)
        .expect("static managed-media response headers are valid")
}

fn request_digest(path: &str) -> Option<&str> {
    let digest = path.strip_prefix('/')?;
    (digest.len() == SHA256_DIGEST_LENGTH
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(digest)
}

fn parse_range(value: &str, object_length: u64) -> Option<ByteRange> {
    let range = value.strip_prefix("bytes=")?;
    if object_length == 0 || range.contains(',') {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix_length = end.parse::<u64>().ok()?;
        if suffix_length == 0 {
            return None;
        }
        return Some(ByteRange {
            start: object_length.saturating_sub(suffix_length),
            end: object_length - 1,
        });
    }

    let start = start.parse::<u64>().ok()?;
    if start >= object_length {
        return None;
    }
    let end = if end.is_empty() {
        object_length - 1
    } else {
        end.parse::<u64>().ok()?.min(object_length - 1)
    };
    (start <= end).then_some(ByteRange { start, end })
}

fn read_body(
    object: &mut File,
    requested_range: Option<ByteRange>,
    content_length: u64,
) -> Result<Vec<u8>, ()> {
    if let Some(range) = requested_range {
        object.seek(SeekFrom::Start(range.start)).map_err(|_| ())?;
    }
    let length = usize::try_from(content_length).map_err(|_| ())?;
    let mut body = vec![0; length];
    object.read_exact(&mut body).map_err(|_| ())?;
    Ok(body)
}

fn range_not_satisfiable(object_length: u64) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(CONTENT_TYPE, AUDIO_MPEG)
        .header(ACCEPT_RANGES, BYTE_RANGE_UNIT)
        .header(
            CONTENT_RANGE,
            format!("{BYTE_RANGE_UNIT} */{object_length}"),
        )
        .header(CONTENT_LENGTH, 0)
        .body(Vec::new())
        .expect("static managed-media response headers are valid")
}

fn empty_response(status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(CONTENT_LENGTH, 0)
        .body(Vec::new())
        .expect("static managed-media response headers are valid")
}

#[cfg(test)]
mod tests {
    use meiki_application::{
        ApplicationService, DirectionDto, ImportMediaRequest, MediaAvailabilityDto, MediaRoleDto,
    };
    use tauri::http::{Request, StatusCode, header};
    use tempfile::tempdir;

    use super::{AUDIO_MPEG, BYTE_RANGE_UNIT, PROTOCOL, response};

    const REAL_MP3: &[u8] = &[
        0x49, 0x44, 0x33, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x36, 0x54, 0x49, 0x54, 0x32, 0x00,
        0x00, 0x00, 0x09, 0x00, 0x00, 0x03, 0x66, 0x69, 0x78, 0x74, 0x75, 0x72, 0x65, 0x00, 0x54,
        0x53, 0x53, 0x45, 0x00, 0x00, 0x00, 0x0f, 0x00, 0x00, 0x03, 0x4c, 0x61, 0x76, 0x66, 0x36,
        0x32, 0x2e, 0x31, 0x32, 0x2e, 0x31, 0x30, 0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0xf3, 0x24, 0xc4, 0x00, 0x07, 0xa0, 0x02, 0xd1, 0xb9, 0x41,
        0x00, 0x02, 0xa9, 0x00, 0x64, 0xbb, 0xf0, 0x01, 0xf0, 0x7c, 0x1f, 0x07, 0xcf, 0x82, 0x11,
        0x38, 0x3e, 0x72, 0xe9, 0x70, 0x7c, 0x1f, 0x07, 0xf8, 0x21, 0xc1, 0xf7, 0xe5, 0x0e, 0x51,
        0xe1, 0xfe, 0xf5, 0x95, 0x46, 0xbf, 0xff, 0xff, 0xf3, 0x24, 0xc4, 0x04, 0x08, 0xa0, 0x7a,
        0xa8, 0x01, 0x99, 0x60, 0x00, 0xdd, 0xa8, 0x8f, 0xd7, 0xfb, 0x28, 0x4d, 0x37, 0x10, 0x43,
        0x81, 0xbc, 0x5a, 0xd2, 0x0d, 0x9c, 0x17, 0x26, 0x98, 0x87, 0x2f, 0xc8, 0x28, 0xf4, 0xb5,
        0x66, 0x9b, 0xca, 0x84, 0x81, 0xa5, 0x34, 0x01, 0xc0, 0x0d, 0xff, 0xf3, 0x24, 0xc4, 0x04,
        0x08, 0xb8, 0x5a, 0x89, 0x99, 0xda, 0x00, 0x02, 0x87, 0xff, 0xff, 0xeb, 0xf5, 0xff, 0x94,
        0xd4, 0xf3, 0xea, 0x20, 0x12, 0x70, 0x81, 0x1f, 0xd1, 0x40, 0x10, 0x09, 0x82, 0x8d, 0xb0,
        0x5c, 0xe8, 0x49, 0xe9, 0xff, 0xff, 0xfb, 0x7f, 0x26, 0x1c, 0x00, 0x3e, 0x51, 0xff, 0xf3,
        0x24, 0xc4, 0x04, 0x06, 0xf8, 0x5e, 0x69, 0xe0, 0x0f, 0x70, 0x49, 0x06, 0x96, 0x55, 0x11,
        0x74, 0x94, 0x34, 0x2a, 0x11, 0x18, 0x95, 0x2e, 0x19, 0x82, 0x1b, 0x25, 0xeb, 0xf4, 0x04,
        0x2b, 0x0f, 0x81, 0xac, 0xad, 0xc2, 0xdc, 0xec, 0x75, 0x98, 0x21, 0x3c, 0x10, 0x01, 0x46,
        0x01, 0xff, 0xf3, 0x24, 0xc4, 0x0b, 0x0a, 0x20, 0x6a, 0x44, 0x00, 0x07, 0xb4, 0x2d, 0x60,
        0x5a, 0x60, 0x68, 0x15, 0xc6, 0x2d, 0xef, 0x14, 0x66, 0x40, 0x1e, 0x46, 0x09, 0xe0, 0x86,
        0x67, 0xd0, 0x9b, 0xcc, 0xc6, 0x2c, 0x80, 0xb0, 0x77, 0x52, 0x5f, 0x63, 0x30, 0x05, 0xd5,
        0xda, 0xd1, 0x67, 0xb5, 0xff, 0xf3, 0x24, 0xc4, 0x05, 0x08, 0x30, 0x66, 0x4c, 0x00, 0x07,
        0x76, 0x2d, 0x8f, 0xa4, 0xc0, 0xd0, 0x46, 0x18, 0x42, 0x1b, 0x9a, 0x12, 0x1f, 0xd8, 0x01,
        0x18, 0xa2, 0x07, 0x98, 0x28, 0xd8, 0x82, 0x08, 0x84, 0x71, 0xa2, 0xc7, 0xae, 0xdc, 0xa9,
        0xc3, 0x00, 0xca, 0x1a, 0x66, 0x02, 0x41, 0xff, 0xf3, 0x24, 0xc4, 0x07, 0x05, 0xc8, 0x5e,
        0x80, 0x18, 0x00, 0xf6, 0x42, 0x93, 0x1d, 0xf7, 0x35, 0xa0, 0x77, 0x7a, 0x95, 0x5b, 0xa2,
        0x16, 0x7b, 0xff, 0xfc, 0xef, 0xf4, 0xaa, 0x00, 0x0d, 0x46, 0xc2, 0x60, 0xaf, 0x10, 0x4c,
        0xb3, 0x06, 0xc5, 0x32, 0xf0, 0x37, 0x22, 0x30, 0xd6, 0xe9, 0xff, 0xf3, 0x24, 0xc4, 0x12,
        0x06, 0x78, 0x5a, 0x88, 0x78, 0x00, 0x76, 0x42, 0x6f, 0x3f, 0xfd, 0x5f, 0x4f, 0xff, 0x77,
        0x23, 0x40, 0x18, 0x0b, 0x56, 0xe7, 0x1b, 0x73, 0x50, 0x6c, 0x6f, 0x5b, 0x24, 0x86, 0x15,
        0x4e, 0x3a, 0x2b, 0xff, 0xff, 0xfd, 0x4a, 0x0a, 0x40, 0x20, 0x80, 0x03, 0xf1, 0xff, 0xf3,
        0x24, 0xc4, 0x1b, 0x05, 0x18, 0x56, 0x88, 0x38, 0x00, 0x74, 0x42, 0xea, 0xac, 0x16, 0xb4,
        0xa5, 0x79, 0x01, 0x25, 0x09, 0x36, 0xb3, 0xa8, 0x8d, 0x70, 0x2f, 0xed, 0x57, 0xff, 0xf5,
        0xfe, 0xaa, 0x11, 0x1f, 0xf4, 0xd3, 0x72, 0x60, 0x80, 0x06, 0x5c, 0x00, 0x1f, 0x00, 0xd8,
        0x83, 0xff, 0xf3, 0x24, 0xc4, 0x29, 0x06, 0xa0, 0x56, 0x9d, 0xd8, 0x00, 0xf4, 0x26, 0xce,
        0x4e, 0x19, 0x26, 0x1f, 0xaa, 0xff, 0xae, 0x57, 0xf3, 0x5f, 0x3e, 0xa9, 0xd4, 0x09, 0xaa,
        0x7f, 0x8e, 0x50, 0x0d, 0xc4, 0xcd, 0x80, 0xeb, 0xbb, 0xf4, 0x0f, 0xc2, 0xc7, 0x1c, 0x6e,
        0x9f, 0x07, 0x16, 0x3f, 0xff, 0xf3, 0x24, 0xc4, 0x31, 0x04, 0xb8, 0x5a, 0x94, 0xc1, 0x54,
        0x00, 0x03, 0x67, 0xb5, 0x30, 0xa6, 0x51, 0x35, 0xbe, 0xff, 0xfa, 0x7a, 0x67, 0x72, 0xc5,
        0x83, 0x16, 0x4f, 0x79, 0x52, 0x21, 0xd6, 0xfe, 0xb1, 0xa3, 0x3f, 0xea, 0x4c, 0x41, 0x4d,
        0x45, 0x33, 0x2e, 0x31, 0x30, 0x30, 0x55, 0xff, 0xf3, 0x24, 0xc4, 0x41, 0x0d, 0xc0, 0xc6,
        0x94, 0x01, 0x99, 0x78, 0x00, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        0xff, 0xf3, 0x24, 0xc4, 0x2d, 0x00, 0x00, 0x03, 0x48, 0x01, 0xc0, 0x00, 0x00, 0x55, 0x55,
        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        0x55, 0x55,
    ];

    #[test]
    fn real_mp3_import_is_served_from_an_extensionless_managed_path_with_mime_and_ranges() {
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("sentence.mp3");
        std::fs::write(&source_path, REAL_MP3).unwrap();
        let collection_path = directory.path().join("collection.db");
        let media = ApplicationService::new(&collection_path)
            .import_media(&ImportMediaRequest {
                path: source_path.to_string_lossy().into_owned(),
                role: MediaRoleDto::PromptAudio,
                language_tag: Some("ko-KR".into()),
                direction: DirectionDto::Auto,
            })
            .unwrap();

        assert_eq!(media.availability, MediaAvailabilityDto::Ready);
        assert_eq!(media.media_type, AUDIO_MPEG);
        let managed_path = media.asset_path.as_ref().unwrap();
        assert!(std::path::Path::new(managed_path).extension().is_none());
        assert_eq!(std::fs::read(managed_path).unwrap(), REAL_MP3);
        let digest = media.content_hash.strip_prefix("sha256:").unwrap();

        let full_request = Request::builder()
            .uri(format!("{PROTOCOL}://localhost/{digest}"))
            .body(Vec::new())
            .unwrap();
        let full_response = response(&collection_path, &full_request);
        assert_eq!(full_response.status(), StatusCode::OK);
        assert_eq!(full_response.headers()[header::CONTENT_TYPE], AUDIO_MPEG);
        assert_eq!(
            full_response.headers()[header::ACCEPT_RANGES],
            BYTE_RANGE_UNIT
        );
        assert_eq!(full_response.body(), REAL_MP3);

        let range_request = Request::builder()
            .uri(format!("{PROTOCOL}://localhost/{digest}"))
            .header(header::RANGE, "bytes=16-63")
            .body(Vec::new())
            .unwrap();
        let range_response = response(&collection_path, &range_request);
        assert_eq!(range_response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            range_response.headers()[header::CONTENT_RANGE],
            format!("bytes 16-63/{}", REAL_MP3.len())
        );
        assert_eq!(range_response.body(), &REAL_MP3[16..64]);
    }

    #[test]
    fn invalid_or_unsatisfiable_managed_media_requests_fail_without_file_access() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let invalid_request = Request::builder()
            .uri(format!("{PROTOCOL}://localhost/not-a-content-hash"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            response(&collection_path, &invalid_request).status(),
            StatusCode::NOT_FOUND
        );

        let source_path = directory.path().join("sentence.mp3");
        std::fs::write(&source_path, REAL_MP3).unwrap();
        let media = ApplicationService::new(&collection_path)
            .import_media(&ImportMediaRequest {
                path: source_path.to_string_lossy().into_owned(),
                role: MediaRoleDto::AnswerAudio,
                language_tag: None,
                direction: DirectionDto::Auto,
            })
            .unwrap();
        let digest = media.content_hash.strip_prefix("sha256:").unwrap();
        let range_request = Request::builder()
            .uri(format!("{PROTOCOL}://localhost/{digest}"))
            .header(header::RANGE, format!("bytes={}-", REAL_MP3.len()))
            .body(Vec::new())
            .unwrap();
        let range_response = response(&collection_path, &range_request);
        assert_eq!(range_response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            range_response.headers()[header::CONTENT_RANGE],
            format!("bytes */{}", REAL_MP3.len())
        );
    }
}
