pub(crate) fn write_wav(path: &str, samples: &[f64], sample_rate: u32) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;

    for &sample in samples {
        writer.write_sample(sample as f32)?;
    }

    writer.finalize()
}
