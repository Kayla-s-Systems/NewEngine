    use super::*;

    fn sine(frames: usize, channels: usize, sample_rate: u32) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * channels);
        for frame in 0..frames {
            let value =
                ((frame as f32 * 440.0 * std::f32::consts::TAU) / sample_rate as f32).sin() * 0.65;
            for channel in 0..channels {
                out.push(if channel == 0 { value } else { value * 0.7 });
            }
        }
        out
    }

    #[test]
    fn writer_emits_parseable_ps_adpcm_xvag() {
        let source = sine(1_000, 2, 48_000);
        let encoded = encode_xvag_ps_adpcm(48_000, 2, &source).expect("encode");
        let header = parse_xvag_header(&encoded).expect("header");
        assert_eq!(header.codec, XvagCodec::PsAdpcm);
        assert_eq!(header.channels, 2);
        assert_eq!(header.sample_rate_hz, 48_000);
        assert_eq!(header.num_samples, 1_000);
        assert_eq!(header.interleave_factor, 1);
        assert_eq!(header.subsongs, 1);
        assert_eq!(header.layers, 1);
        assert_eq!(*encoded.last().unwrap(), 0);
    }

    #[test]
    fn ps_adpcm_round_trip_preserves_length_and_reasonable_signal_quality() {
        let source = sine(4_800, 1, 48_000);
        let encoded = encode_xvag_ps_adpcm(48_000, 1, &source).expect("encode");
        let decoded = decode_xvag_ps_adpcm(&encoded).expect("decode");
        assert_eq!(decoded.sample_rate_hz, 48_000);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples.len(), source.len());
        let mse = source
            .iter()
            .zip(&decoded.samples)
            .map(|(a, b)| {
                let d = f64::from(*a - *b);
                d * d
            })
            .sum::<f64>()
            / source.len() as f64;
        assert!(mse < 0.0025, "unexpectedly high PS-ADPCM mse={mse}");
    }

    #[test]
    fn atrac9_info_reads_big_endian_config_word_inside_le_xvag() {
        let mut encoded = encode_xvag_ps_adpcm(48_000, 1, &sine(100, 1, 48_000)).unwrap();
        encoded[0x2c..0x30].copy_from_slice(&XVAG_CODEC_ATRAC9.to_le_bytes());
        encoded[0x4c..0x50].copy_from_slice(b"a9in");
        encoded[0x50..0x54].copy_from_slice(&(0x18_u32).to_le_bytes());
        encoded.resize(0x68.max(encoded.len()), 0);
        encoded[0x54..0x58].copy_from_slice(&(96_u32).to_le_bytes());
        encoded[0x58..0x5c].copy_from_slice(&(256_u32).to_le_bytes());
        encoded[0x5c..0x60].copy_from_slice(&0x1234_5678_u32.to_be_bytes());
        encoded[0x60..0x64].copy_from_slice(&(100_u32).to_le_bytes());
        encoded[0x64..0x68].copy_from_slice(&(7_u32).to_le_bytes());
        // Preserve source data after growing the metadata area for this synthetic fixture.
        encoded[0x04..0x08].copy_from_slice(&(0x68_u32).to_le_bytes());
        let data = vec![0_u8; 96];
        encoded.extend_from_slice(&data);
        encoded[0x40..0x44].copy_from_slice(&(96_u32).to_le_bytes());
        let inspection = inspect_xvag(&encoded).expect("inspect ATRAC9");
        let XvagCodecInfo::Atrac9(info) = inspection.codec_info else {
            panic!("expected ATRAC9 info")
        };
        assert_eq!(info.frame_size, 96);
        assert_eq!(info.samples_per_frame, 256);
        assert_eq!(info.config_data_be, 0x1234_5678);
    }

    #[test]
    fn demux_descriptor_exposes_ps_adpcm_frame_and_interleave_contract() {
        let encoded = encode_xvag_ps_adpcm(48_000, 2, &sine(280, 2, 48_000)).unwrap();
        let demux = xvag_demux_descriptor(&encoded, 1, 1).expect("demux");
        assert_eq!(demux.codec, XvagCodec::PsAdpcm);
        assert_eq!(demux.codec_frame_bytes, 16);
        assert_eq!(demux.interleave_block_bytes, 16);
        assert_eq!(demux.channels_per_layer, 2);
        assert_eq!(demux.stream_count, 1);
    }

    #[test]
    fn rejects_codec_without_native_decoder() {
        let mut encoded = encode_xvag_ps_adpcm(48_000, 1, &sine(100, 1, 48_000)).unwrap();
        encoded[0x2c..0x30].copy_from_slice(&XVAG_CODEC_ATRAC9.to_le_bytes());
        let error = decode_xvag_ps_adpcm(&encoded).unwrap_err();
        assert!(error.contains("not supported"));
    }
