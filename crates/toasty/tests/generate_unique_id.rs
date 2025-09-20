use toasty::codegen_support::generate_unique_id;

#[test]
fn test_generate_unique_id() {
    let id1 = generate_unique_id();
    let id2 = generate_unique_id();
    let id3 = generate_unique_id();

    // IDs should be unique and sequential
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // Should be sequential (though we can't guarantee exact values due to other tests)
    assert!(id1.0 < id2.0);
    assert!(id2.0 < id3.0);
}

#[test]
fn test_generate_unique_id_thread_safety() {
    use std::thread;

    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(|| {
                let mut ids = Vec::new();
                for _ in 0..100 {
                    ids.push(generate_unique_id());
                }
                ids
            })
        })
        .collect();

    let mut all_ids = Vec::new();
    for handle in handles {
        let ids = handle.join().unwrap();
        all_ids.extend(ids);
    }

    // All IDs should be unique
    all_ids.sort_by_key(|id| id.0);
    for window in all_ids.windows(2) {
        assert_ne!(window[0], window[1], "Found duplicate ID: {:?}", window[0]);
    }
}
