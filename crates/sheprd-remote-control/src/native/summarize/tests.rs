use super::*;

#[test]
fn a_summary_is_dated_when_its_transcript_was_read() {
    // A turn that ends after the read, at 150, is newer than the summary,
    // so the row shows that reply instead.
    assert_eq!(
        finished(Ok("Fixing the login bug.".into()), 100),
        RowSummary::Ready {
            text: "Fixing the login bug.".into(),
            at_ms: 100,
        }
    );
}

#[test]
fn a_failure_is_dated_when_its_transcript_was_read() {
    assert_eq!(
        finished(Err(row_summary::Error::TimedOut), 100),
        RowSummary::Failed {
            reason: row_summary::Error::TimedOut.to_string(),
            at_ms: 100,
        }
    );
}
