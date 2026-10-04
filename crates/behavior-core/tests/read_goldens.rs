#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Golden read records (feature 010, FR-010): every request in `tests/fixtures/reads/requests`
//! gives exactly its reviewed record, and the record replays. Run with `BLESS_READ_GOLDENS=1` to
//! write missing records (review them before committing).

mod common;

use behavior_core::admit;
use behavior_core::read::{evaluate_read_request, replay_read};

#[test]
fn read_records_equal_their_goldens() {
    let m = admit(&common::read(
        &common::fixtures().join("reads/modules/lab.json"),
    ))
    .unwrap();
    let bless = std::env::var("BLESS_READ_GOLDENS").is_ok();
    let dir = common::fixtures().join("reads/requests");
    let out = common::fixtures().join("reads/records");
    let requests = common::files(&dir, ".json");
    assert!(requests.len() >= 8, "{} requests", requests.len());
    let mut failures = Vec::new();
    for path in requests {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let x = evaluate_read_request(&m, &common::read(&path))
            .unwrap_or_else(|e| panic!("{name}: {:?}", e.errors));
        let got = x.record.to_json_string();
        assert!(replay_read(&m, &got).matches, "{name} does not replay");
        let golden = out.join(format!("{name}.expected.json"));
        match std::fs::read_to_string(&golden) {
            Ok(want) if want == got => {}
            Ok(want) => failures.push(format!("{name}\n  want {want}\n  got  {got}")),
            Err(_) if bless => {
                std::fs::create_dir_all(&out).unwrap();
                std::fs::write(&golden, &got).unwrap();
            }
            Err(_) => failures.push(format!("{name}: no golden (run with BLESS_READ_GOLDENS=1)")),
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
