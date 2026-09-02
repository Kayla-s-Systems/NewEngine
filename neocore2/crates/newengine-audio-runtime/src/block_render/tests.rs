use super::*;
use rodio::buffer::SamplesBuffer;

fn mono(samples: &[f32]) -> SamplesBuffer {
    SamplesBuffer::new(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
        samples.to_vec(),
    )
}

#[test]
fn master_source_mixes_multiple_nodes_inside_one_preallocated_block() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    graph
        .add_source(mono(&[1.0, 1.0, 1.0]), 0.5, 1.0, false, Duration::ZERO)
        .unwrap();
    graph
        .add_source(mono(&[0.25, 0.25, 0.25]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    assert!((source.next().unwrap() - 0.75).abs() < 1.0e-6);
    assert!((source.next().unwrap() - 0.75).abs() < 1.0e-6);
}

#[test]
fn scheduled_gain_command_splits_block_at_exact_output_sample() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    let handle = graph
        .add_source(mono(&vec![1.0; 512]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    handle.schedule_gain_at(32, 0.0, 0, 1).unwrap();
    let rendered = (0..64).map(|_| source.next().unwrap()).collect::<Vec<_>>();
    assert!(rendered[..32]
        .iter()
        .all(|sample| (*sample - 1.0).abs() < 1.0e-6));
    assert!(rendered[32..].iter().all(|sample| sample.abs() < 1.0e-6));
    assert!(graph.stats().split_segments >= 1);
}

#[test]
fn scheduled_add_starts_source_on_exact_frame_boundary() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    graph
        .add_boxed_source(
            Box::new(mono(&vec![1.0; 32])),
            1.0,
            1.0,
            false,
            Duration::ZERO,
            Some(17),
        )
        .unwrap();
    let rendered = (0..32).map(|_| source.next().unwrap()).collect::<Vec<_>>();
    assert!(rendered[..17].iter().all(|sample| sample.abs() < 1.0e-6));
    assert!(rendered[17..]
        .iter()
        .all(|sample| (*sample - 1.0).abs() < 1.0e-6));
}

#[test]
fn cancelled_future_add_never_becomes_audible() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    let handle = graph
        .add_boxed_source(
            Box::new(mono(&vec![1.0; 64])),
            1.0,
            1.0,
            false,
            Duration::ZERO,
            Some(24),
        )
        .unwrap();
    handle.stop();
    let rendered = (0..48).map(|_| source.next().unwrap()).collect::<Vec<_>>();
    assert!(rendered.iter().all(|sample| sample.abs() < 1.0e-6));
    assert!(handle.empty());
}

#[test]
fn cancelled_scheduled_gain_does_not_fire() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    let handle = graph
        .add_source(mono(&vec![1.0; 128]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    handle.schedule_gain_at(32, 0.0, 0, 77).unwrap();
    handle.cancel_scheduled(77).unwrap();
    let rendered = (0..64).map(|_| source.next().unwrap()).collect::<Vec<_>>();
    assert!(rendered.iter().all(|sample| (*sample - 1.0).abs() < 1.0e-6));
}

#[test]
fn paused_node_does_not_advance_source_clock() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    let handle = graph
        .add_source(mono(&vec![1.0; 512]), 1.0, 1.0, true, Duration::ZERO)
        .unwrap();
    for _ in 0..64 {
        let _ = source.next();
    }
    assert_eq!(handle.get_pos(), Duration::ZERO);
}

#[test]
fn same_sample_commands_preserve_submission_sequence() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    let handle = graph
        .add_source(mono(&vec![1.0; 128]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    handle.schedule_gain_at(32, 0.0, 0, 100).unwrap();
    handle.schedule_gain_at(32, 0.5, 0, 101).unwrap();
    let rendered = (0..64).map(|_| source.next().unwrap()).collect::<Vec<_>>();
    assert!(rendered[..32]
        .iter()
        .all(|sample| (*sample - 1.0).abs() < 1.0e-6));
    assert!(rendered[32..]
        .iter()
        .all(|sample| (*sample - 0.5).abs() < 1.0e-6));
}

#[test]
fn master_output_limiter_bounds_overloaded_voice_sum_without_touching_unity() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    graph
        .add_source(mono(&[1.0, 1.0]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    graph
        .add_source(mono(&[1.0, 1.0]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    assert!((source.next().unwrap() - 1.0).abs() < 1.0e-6);
}

#[test]
fn master_output_limiter_is_channel_linked() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(2).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    let stereo = SamplesBuffer::new(
        ChannelCount::new(2).unwrap(),
        SampleRate::new(48_000).unwrap(),
        vec![2.0, 1.0],
    );
    graph
        .add_source(stereo, 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    let left = source.next().unwrap();
    let right = source.next().unwrap();
    assert!((left - 1.0).abs() < 1.0e-6, "left={left}");
    assert!((right - 0.5).abs() < 1.0e-6, "right={right}");
}

#[test]
fn dense_node_compaction_preserves_moved_voice_command_lookup() {
    let (graph, mut source) = native_block_render_graph(
        ChannelCount::new(1).unwrap(),
        SampleRate::new(48_000).unwrap(),
    );
    graph
        .add_source(mono(&vec![0.1; 768]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    let middle = graph
        .add_source(mono(&vec![0.2; 768]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();
    let moved = graph
        .add_source(mono(&vec![0.4; 768]), 1.0, 1.0, false, Duration::ZERO)
        .unwrap();

    assert!((source.next().unwrap() - 0.7).abs() < 1.0e-6);
    middle.stop();
    moved.set_volume(0.5);

    // Finish the already-rendered block. The remove + gain commands are consumed at the
    // next block boundary, where removing the middle node swap-moves the last node.
    for _ in 1..NATIVE_BLOCK_FRAMES {
        let _ = source.next().unwrap();
    }
    let next = source.next().unwrap();
    assert!((next - 0.3).abs() < 1.0e-6, "next={next}");
    assert_eq!(graph.stats().active_nodes, 2);
}
