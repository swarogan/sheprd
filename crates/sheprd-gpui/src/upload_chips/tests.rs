use super::{Sending, Upload, split, totals};
use std::{path::PathBuf, sync::atomic::Ordering};

#[test]
fn bytes_fill_files_in_sending_order() {
    let sizes = [100, 50, 10];
    assert_eq!(
        split(0, &sizes),
        [
            Sending::Waiting(Some(100)),
            Sending::Waiting(Some(50)),
            Sending::Waiting(Some(10))
        ]
    );
    assert_eq!(
        split(125, &sizes),
        [
            Sending::Sent,
            Sending::Partly(0.5),
            Sending::Waiting(Some(10))
        ]
    );
    assert_eq!(split(160, &sizes), [Sending::Sent; 3]);
    // A report past the total, or a file grown since it was measured,
    // still reads as sent rather than past full.
    assert_eq!(split(500, &sizes), [Sending::Sent; 3]);
}

#[test]
fn an_empty_file_is_sent_once_the_ones_before_it_are() {
    assert_eq!(
        split(0, &[0, 10]),
        [Sending::Sent, Sending::Waiting(Some(10))]
    );
    assert_eq!(
        split(5, &[10, 0, 10]),
        [
            Sending::Partly(0.5),
            Sending::Waiting(Some(0)),
            Sending::Waiting(Some(10))
        ]
    );
    assert_eq!(
        split(10, &[10, 0, 10]),
        [Sending::Sent, Sending::Sent, Sending::Waiting(Some(10))]
    );
}

#[test]
fn files_wait_without_a_size_until_the_worker_reads_them() {
    let upload = Upload::new(&[
        PathBuf::from("/work/a.log"),
        PathBuf::from("/work/shot.png"),
    ]);
    let files: Vec<_> = upload.files().collect();
    assert_eq!(
        files,
        [
            ("a.log", Sending::Waiting(None)),
            ("shot.png", Sending::Waiting(None))
        ]
    );
    assert_eq!(upload.bytes(), None);

    let upload = Upload::sample(&["a.log", "shot.png"], vec![40, 60], 70);
    assert_eq!(upload.bytes(), Some((70, 100)));
    assert_eq!(
        upload.files().nth(1),
        Some(("shot.png", Sending::Partly(0.5)))
    );
}

#[test]
fn totals_count_only_uploads_whose_sizes_are_known() {
    let measured = Upload::sample(&["a.log"], vec![40], 10);
    let unmeasured = Upload::new(&[PathBuf::from("/work/b.log")]);
    assert_eq!(totals([&measured, &unmeasured]), Some((10, 40)));
    assert_eq!(totals([&unmeasured]), None);
}

#[test]
fn a_cancel_reaches_the_worker() {
    let upload = Upload::new(&[PathBuf::from("/work/a.log")]);
    let reporter = upload.reporter();
    assert!(upload.is(&reporter));
    assert!(!reporter.cancelled().load(Ordering::Acquire));
    upload.cancel();
    assert!(reporter.cancelled().load(Ordering::Acquire));
    assert!(upload.cancelled());
}

#[test]
fn the_reporter_says_when_sizes_are_known_and_a_cancel_after_the_copy() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    std::io::Write::write_all(&mut file, &[0; 300]).unwrap();
    let upload = Upload::new(&[file.path().to_owned()]);
    let reporter = upload.reporter();
    assert!(!reporter.measured());
    reporter.measure(&[file.path().to_owned()]);
    assert!(reporter.measured());
    assert_eq!(upload.bytes(), Some((0, 300)));

    // The copy may finish before the click lands; the cancel still counts.
    reporter.sent(300);
    reporter.finish();
    upload.cancel();
    assert!(reporter.was_cancelled());
}
