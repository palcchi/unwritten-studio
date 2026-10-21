use unwritten_studio_aep::{Limits, parse, parse_with_limits};

fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    bytes.extend_from_slice(data);
    if data.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}
fn container(payload: &[u8]) -> Vec<u8> {
    let mut bytes = b"RIFX".to_vec();
    bytes.extend_from_slice(&((payload.len() + 4) as u32).to_be_bytes());
    bytes.extend_from_slice(b"Egg!");
    bytes.extend_from_slice(payload);
    bytes
}
#[test]
fn accepts_empty_container_but_no_invented_project() {
    let bytes = container(&[]);
    let file = parse(&bytes).unwrap();
    assert!(unwritten_studio_aep::Project::new(&file).is_none());
}
#[test]
fn rejects_truncation_and_wrong_headers() {
    let good = container(&chunk(b"Utf8", b"Example"));
    for end in 0..good.len() {
        assert!(parse(&good[..end]).is_err(), "prefix {end}");
    }
    assert!(parse(b"not an AEP file").is_err());
    let mut bad = good.clone();
    bad[8..12].copy_from_slice(b"WAVE");
    assert!(parse(&bad).is_err());
}
#[test]
fn enforces_chunk_budget() {
    let mut payload = chunk(b"Utf8", b"one");
    payload.extend(chunk(b"Utf8", b"two"));
    assert!(parse_with_limits(&container(&payload), Limits { max_depth: 64, max_chunks: 1 }).is_err());
}
#[test]
fn bounded_depth_keeps_deep_lists_opaque() {
    let mut payload = chunk(b"Utf8", b"leaf");
    for _ in 0..80 {
        let mut list = b"Test".to_vec();
        list.extend(payload);
        payload = chunk(b"LIST", &list);
    }
    assert!(parse_with_limits(&container(&payload), Limits { max_depth: 8, max_chunks: 100 }).is_ok());
}
#[test]
fn arbitrary_small_inputs_never_panic() {
    let mut state = 0x87654321u32;
    for size in 0..1024 {
        let mut bytes = vec![0; size];
        for b in &mut bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *b = state as u8;
        }
        if size >= 12 && size % 2 == 0 {
            bytes[..4].copy_from_slice(b"RIFX");
            bytes[4..8].copy_from_slice(&((size - 8) as u32).to_be_bytes());
            bytes[8..12].copy_from_slice(b"Egg!");
        }
        let _ = parse_with_limits(&bytes, Limits { max_depth: 8, max_chunks: 100 });
    }
}
