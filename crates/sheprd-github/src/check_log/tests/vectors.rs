//! The iOS app cuts failing logs itself; both must cut the same excerpt
//! from the vectors its tests read too.

use super::*;

#[derive(serde::Deserialize)]
struct Vectors {
    max_excerpt: usize,
    vectors: Vec<Vector>,
}

#[derive(serde::Deserialize)]
struct Vector {
    name: String,
    log: String,
    excerpt: String,
}

#[test]
fn the_phone_and_the_computer_cut_the_same_excerpts() {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../../../ios/SheprdRemoteCore/Tests/SheprdGitHubTests/Fixtures/check_log_vectors.json"
    ))
    .unwrap();
    assert_eq!(vectors.max_excerpt, MAX_EXCERPT);
    assert!(!vectors.vectors.is_empty());
    for vector in vectors.vectors {
        assert_eq!(excerpt(&vector.log), vector.excerpt, "{}", vector.name);
    }
}
