// SPDX-License-Identifier: MIT
// Copyright (c) 2026 handy-computer. handy-recorder 0.1.0, vendored by Wispr Bro (see Cargo.toml).
// Added by Wispr Bro contributors, 2026-10-30 (DEC-53, S-01).
//! `RecorderConfig::pause_when_idle`: the stream is built at open, runs only
//! between `start` and `stop`, and every recording still gets exactly its
//! own audio.

use std::time::Duration;

use super::support::{self, Chunks, fake, open, open_notified, ramp, start, stop_with_boundary};
use crate::backend::{BackendError, BackendErrorKind};
use crate::{ErrorKind, Recorder, RecorderConfig};

fn prepared() -> RecorderConfig {
    RecorderConfig {
        frames_per_chunk: Some(160),
        pause_when_idle: true,
        ..RecorderConfig::default()
    }
}

#[test]
fn open_builds_the_stream_without_running_it() {
    let fake = fake(16_000, 1);
    let recorder: Recorder<Chunks> = open(&fake, prepared());
    assert!(fake.is_streaming(), "the stream is built at open");
    assert!(!fake.is_running(), "but not started");
    assert!(
        !fake.push(&[0.5f32; 16]),
        "an idle prepared stream gets no blocks"
    );
    assert_eq!(fake.streams_started(), 1);
    drop(recorder);
    assert!(!fake.is_streaming());
}

#[test]
fn start_runs_the_stream_and_stop_stops_it_again() {
    let fake = fake(16_000, 1);
    let recorder: Recorder<Chunks> = open(&fake, prepared());
    for cycle in 0..4 {
        start(&recorder, Chunks::default());
        assert!(fake.is_running(), "cycle {cycle}: running while recording");
        let audio = ramp(cycle * 100, 321);
        assert!(fake.push(&audio));
        let boundary = [0.25f32];
        let stopped = stop_with_boundary(&recorder, &fake, &boundary).unwrap();
        assert!(stopped.is_complete());
        let mut expected = audio;
        expected.extend_from_slice(&boundary);
        assert_eq!(stopped.sink.real(), expected, "cycle {cycle}");
        support::wait_until("stream stopped after the recording", || !fake.is_running());
    }
    // Built once, started and stopped per recording.
    assert_eq!(fake.streams_started(), 1);
    assert_eq!(
        fake.run_calls(),
        vec![true, false, true, false, true, false, true, false]
    );
}

#[test]
fn a_stream_without_pause_when_idle_runs_from_open_and_is_never_toggled() {
    let fake = fake(16_000, 1);
    let recorder: Recorder<Chunks> = open(
        &fake,
        RecorderConfig {
            frames_per_chunk: Some(160),
            ..RecorderConfig::default()
        },
    );
    assert!(fake.is_running());
    start(&recorder, Chunks::default());
    assert!(fake.push(&[0.5f32; 10]));
    stop_with_boundary(&recorder, &fake, &[0.0f32]).unwrap();
    assert!(fake.is_running());
    assert!(fake.run_calls().is_empty());
}

#[test]
fn audio_queued_before_a_prepared_start_is_not_part_of_the_recording() {
    let fake = fake(16_000, 1);
    let recorder: Recorder<Chunks> = open(&fake, prepared());
    // First recording; the stop's resume lets late blocks into the ring
    // before the stream stops.
    start(&recorder, Chunks::default());
    assert!(fake.push(&ramp(0, 50)));
    stop_with_boundary(&recorder, &fake, &[0.5f32]).unwrap();
    // The next recording begins with its own first block only.
    start(&recorder, Chunks::default());
    let audio = ramp(700, 40);
    assert!(fake.push(&audio));
    let stopped = stop_with_boundary(&recorder, &fake, &[0.125f32]).unwrap();
    let mut expected = audio;
    expected.push(0.125);
    assert_eq!(stopped.sink.real(), expected);
}

#[test]
fn a_stream_that_will_not_start_fails_the_recorder_and_hands_the_sink_back() {
    let fake = fake(16_000, 1);
    let (recorder, failures) = open_notified::<Chunks>(&fake, prepared(), support::timeouts());
    fake.fail_next_run(BackendError::new(
        BackendErrorKind::DeviceNotAvailable,
        "the fake device went away",
    ));
    let error = recorder.start(Chunks::default()).unwrap_err();
    assert_eq!(error.error.kind(), ErrorKind::DeviceLost);
    let _sink: Chunks = error.sink;
    let notified = failures
        .recv_timeout(Duration::from_secs(5))
        .expect("the failure handler runs");
    assert_eq!(notified.kind(), ErrorKind::DeviceLost);
    // The recorder has failed: the next start reports it at once.
    let again = recorder.start(Chunks::default()).unwrap_err();
    assert_eq!(again.error.kind(), ErrorKind::DeviceLost);
}

#[test]
fn an_idle_prepared_stream_never_trips_the_no_audio_bound() {
    let fake = fake(16_000, 1);
    let mut timeouts = support::timeouts();
    timeouts.no_audio = Duration::from_millis(60);
    timeouts.stall = Duration::from_millis(60);
    let (recorder, failures) = open_notified::<Chunks>(&fake, prepared(), timeouts);
    // Several no-audio bounds pass while idle: not a failure.
    assert!(failures.recv_timeout(Duration::from_millis(300)).is_err());
    // A started stream that delivers nothing does fail.
    recorder
        .start(Chunks::default())
        .map_err(|e| e.error)
        .unwrap();
    let error = failures
        .recv_timeout(Duration::from_secs(5))
        .expect("no audio after start fails the recorder");
    assert_eq!(error.kind(), ErrorKind::NoAudio);
}
