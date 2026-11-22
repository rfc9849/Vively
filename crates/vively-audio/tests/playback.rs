use std::{
    io::{Read, Seek, SeekFrom},
    time::Duration,
};

use url::Url;
use vively_audio::{
    DualAudioEngine, EngineTickOutcome, StreamingAudioBuffer, TrackSource,
    effective_crossfade_secs, linear_crossfade_gains, prepare_track, stream_url_into_buffer,
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

fn packet(pid: u16, payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![
        0x47,
        0x40 | ((pid >> 8) as u8 & 31),
        pid as u8,
        0x30,
        (183 - payload.len()) as u8,
    ];
    packet.resize(188 - payload.len(), 0);
    packet.extend_from_slice(payload);
    packet
}

fn ts_segment(audio_bytes: &[u8]) -> Vec<u8> {
    let pat = [0, 0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe1, 0, 0, 0, 0, 0];
    let pmt = [
        0, 2, 0xb0, 18, 0, 1, 0xc1, 0, 0, 0xe1, 1, 0xf0, 0, 3, 0xe1, 1, 0xf0, 0, 0, 0, 0, 0,
    ];
    let mut out = Vec::new();
    out.extend_from_slice(&packet(0, &pat));
    out.extend_from_slice(&packet(256, &pmt));
    for (idx, chunk) in audio_bytes.chunks(160).enumerate() {
        if idx == 0 {
            let mut pes = vec![0, 0, 1, 0xc0, 0, 0, 0x80, 0, 0];
            pes.extend_from_slice(chunk);
            out.extend_from_slice(&packet(257, &pes));
        } else {
            let mut cont = vec![0x47, 0x01, 0x01, 0x30, (183 - chunk.len()) as u8];
            cont.resize(188 - chunk.len(), 0);
            cont.extend_from_slice(chunk);
            out.extend_from_slice(&cont);
        }
    }
    out
}

fn silent_mp3_frame() -> Vec<u8> {
    let mut frame = vec![0xff, 0xfb, 0x90, 0x64];
    frame.resize(417, 0);
    frame
}

#[test]
fn crossfade_math_shortens_on_brief_tracks_and_ramps_linearly() {
    assert_eq!(effective_crossfade_secs(0.0, 200.0), 0.0);
    assert_eq!(effective_crossfade_secs(6.0, 3.0), 0.0);
    assert_eq!(effective_crossfade_secs(6.0, 10.0), 4.0);
    assert_eq!(effective_crossfade_secs(6.0, 180.0), 6.0);
    assert_eq!(effective_crossfade_secs(20.0, 180.0), 12.0);

    let (out0, in0) = linear_crossfade_gains(0.0, 0.8);
    assert!((out0 - 0.8).abs() < 1e-6);
    assert!((in0 - 0.0).abs() < 1e-6);

    let (out_mid, in_mid) = linear_crossfade_gains(0.5, 0.8);
    assert!((out_mid - 0.4).abs() < 1e-6);
    assert!((in_mid - 0.4).abs() < 1e-6);

    let (out1, in1) = linear_crossfade_gains(1.0, 0.8);
    assert!((out1 - 0.0).abs() < 1e-6);
    assert!((in1 - 0.8).abs() < 1e-6);
}

#[tokio::test]
async fn hls_stream_demuxes_segments_in_order_and_supports_seek() {
    let server = MockServer::start().await;
    let frame = silent_mp3_frame();
    let seg1_mp3 = frame.repeat(40);
    let seg2_mp3 = frame.repeat(30);

    let m3u8 = format!(
        "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:0\n{}/seg0.ts\n{}/seg1.ts\n",
        server.uri(),
        server.uri()
    );
    Mock::given(path("/stream.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(m3u8))
        .mount(&server)
        .await;
    Mock::given(path("/seg0.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(ts_segment(&seg1_mp3)))
        .mount(&server)
        .await;
    Mock::given(path("/seg1.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(ts_segment(&seg2_mp3)))
        .mount(&server)
        .await;

    let buffer = StreamingAudioBuffer::new();
    let url = Url::parse(&format!("{}/stream.m3u8", server.uri())).unwrap();
    stream_url_into_buffer(reqwest::Client::new(), url, buffer.clone()).await;

    let expected_len = seg1_mp3.len() + seg2_mp3.len();
    assert_eq!(buffer.buffered_len(), expected_len);
    assert_eq!(buffer.complete_len(), Some(expected_len as u64));

    let mut reader = buffer.reader();
    let mut head = [0u8; 4];
    reader.read_exact(&mut head).unwrap();
    assert_eq!(head, [0xff, 0xfb, 0x90, 0x64]);

    let pos = reader.seek(SeekFrom::Start(417 * 10)).unwrap();
    assert_eq!(pos, 417 * 10);
    reader.read_exact(&mut head).unwrap();
    assert_eq!(head, [0xff, 0xfb, 0x90, 0x64]);
    assert_eq!(buffer.format_hint(), Some("mp3"));
}

#[tokio::test]
async fn hls_adts_aac_stream_demuxes_0x0f_and_detects_aac_hint() {
    let server = MockServer::start().await;
    let pat = [0, 0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe1, 0, 0, 0, 0, 0];
    let pmt_aac = [
        0, 2, 0xb0, 18, 0, 1, 0xc1, 0, 0, 0xe1, 1, 0xf0, 0, 0x0f, 0xe1, 1, 0xf0, 0, 0, 0, 0, 0,
    ];
    let adts_payload = [0xff, 0xf1, 0x50, 0x80, 0x01, 0xdf, 0xfc, 0x21, 0x00].repeat(40);
    let mut pes = vec![0, 0, 1, 0xc0, 0, 0, 0x80, 0, 0];
    pes.extend_from_slice(&adts_payload[..90]);
    let seg0 = [packet(0, &pat), packet(256, &pmt_aac), packet(257, &pes)].concat();
    let seg1 = packet(257, &pes);

    let m3u8 = format!(
        "#EXTM3U
#EXT-X-MEDIA-SEQUENCE:0
{}/a0.ts
{}/a1.ts
",
        server.uri(),
        server.uri()
    );
    Mock::given(path("/aac.m3u8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(m3u8))
        .mount(&server)
        .await;
    Mock::given(path("/a0.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(seg0))
        .mount(&server)
        .await;
    Mock::given(path("/a1.ts"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(seg1))
        .mount(&server)
        .await;

    let buffer = StreamingAudioBuffer::new();
    let url = Url::parse(&format!("{}/aac.m3u8", server.uri())).unwrap();
    stream_url_into_buffer(reqwest::Client::new(), url, buffer.clone()).await;

    assert_eq!(buffer.buffered_len(), 180);
    assert_eq!(buffer.format_hint(), Some("aac"));
}

#[test]
fn real_engine_opens_macos_hardware_sink_with_default_buffer_size() {
    use rodio::cpal::{self, traits::HostTrait as _};
    if cpal::default_host().default_output_device().is_some() {
        let engine = DualAudioEngine::new().unwrap();
        assert!(engine.has_hardware_output());
    }
}

#[tokio::test]
async fn simulated_dual_engine_preloads_seeks_and_promotes_standby() {
    let dir = tempfile::tempdir().unwrap();
    let file_a = dir.path().join("a.mp3");
    let file_b = dir.path().join("b.mp3");
    let mp3 = silent_mp3_frame().repeat(80);
    std::fs::write(&file_a, &mp3).unwrap();
    std::fs::write(&file_b, &mp3).unwrap();

    let handle = tokio::runtime::Handle::current();
    let mut engine = DualAudioEngine::new_simulated().unwrap();
    let prep_a = prepare_track(
        &handle,
        engine.http_client(),
        "7_1".into(),
        20.0,
        TrackSource::LocalFile(file_a),
    )
    .await
    .unwrap();
    let prep_b = prepare_track(
        &handle,
        engine.http_client(),
        "7_2".into(),
        25.0,
        TrackSource::LocalFile(file_b),
    )
    .await
    .unwrap();

    engine.load_active(prep_a, Some(0), true, 0.0).unwrap();
    engine.preload_standby(prep_b, 1).unwrap();
    assert!(engine.has_valid_standby(1, "7_2"));
    assert_eq!(engine.active_track_id(), Some("7_1"));

    assert!(engine.seek(16.0));
    let outcome = engine.tick(6.0);
    assert_eq!(outcome, EngineTickOutcome::PromotedStandby { queue_index: 1 });
    assert_eq!(engine.active_track_id(), Some("7_2"));
    assert!(engine.is_crossfading());

    engine.pause();
    assert_eq!(engine.tick(6.0), EngineTickOutcome::Idle);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let paused_pos = engine.position();
    assert!(paused_pos < 0.2);
}
