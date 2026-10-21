//! Synthetic writer regressions; no proprietary or private fixture bytes.
use std::borrow::Cow;
use unwritten_studio_aep::{AepError, AepFile, Body, Chunk, parse, write, write_to};

#[test]
fn writer_preserves_opaque_lists_nested_names_padding_and_trailer() {
    let file = AepFile {
        chunks: vec![
            Chunk::data(*b"odd!", vec![1, 2, 3]),
            Chunk::list(*b"Fold", vec![Chunk::list(*b"Item", vec![Chunk::data(*b"Utf8", "日本語".as_bytes().to_vec())])]),
            Chunk { id: *b"LIST", body: Body::OpaqueList { list_type: *b"btdk", data: Cow::Owned(b"opaque text".to_vec()) }, pad: Some(0) },
        ],
        trailer: Cow::Borrowed(b"private-free synthetic trailer"),
    };
    let bytes = write(&file).unwrap();
    assert_eq!(bytes.get(..4), Some(b"RIFX".as_slice()));
    assert_eq!(bytes.get(8..12), Some(b"Egg!".as_slice()));
    let parsed = parse(&bytes).unwrap();
    assert_eq!(write(&parsed).unwrap(), bytes);
    let mut streamed = Vec::new();
    write_to(&file, &mut streamed).unwrap();
    assert_eq!(streamed, bytes);
}

#[test]
fn writer_refuses_excessive_tree_depth_without_panicking() {
    let mut node = Chunk::data(*b"leaf", vec![0]);
    for _ in 0..260 {
        node = Chunk::list(*b"deep", vec![node]);
    }
    let file = AepFile { chunks: vec![node], trailer: Cow::Borrowed(&[]) };
    assert!(matches!(write(&file), Err(AepError::TooDeep)));
}

#[test]
fn odd_final_chunks_without_padding_remain_lossless() {
    let file = AepFile { chunks: vec![Chunk { id: *b"last", body: Body::Data(Cow::Borrowed(b"abc")), pad: None }], trailer: Cow::Borrowed(&[]) };
    let bytes = write(&file).unwrap();
    assert_eq!(write(&parse(&bytes).unwrap()).unwrap(), bytes);
    assert_eq!(bytes.len(), 23);
}

#[test]
fn writer_reports_output_io_failure() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic full disk"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let file = AepFile { chunks: vec![], trailer: Cow::Borrowed(&[]) };
    assert!(write_to(&file, &mut Broken).is_err());
}

#[test]
fn writer_rejects_oversized_borrowed_tree_before_copying() {
    // Only 1 MiB backing storage; the output would exceed 64 MiB if copied.
    let backing = vec![0u8; 1024 * 1024];
    let chunks = (0..65).map(|_| Chunk::data(*b"data", backing.as_slice())).collect();
    let file = AepFile { chunks, trailer: Cow::Borrowed(&[]) };
    assert!(matches!(write(&file), Err(AepError::TooLarge)));
    let mut target = Vec::new();
    assert!(write_to(&file, &mut target).is_err());
    assert!(target.is_empty(), "preflight must complete before output is written");
}
