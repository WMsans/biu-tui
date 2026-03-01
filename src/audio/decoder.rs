use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;

pub struct AudioDecoder {
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
}

impl AudioDecoder {
    pub fn from_url(url: &str) -> Result<Self> {
        ffmpeg::init()?;

        let mut ictx = ffmpeg::format::input(&url)
            .with_context(|| format!("Failed to open input: {}", url))?;

        let input = ictx
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .context("Could not find audio stream")?;
        let stream_index = input.index();

        let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
        let decoder = context_decoder
            .decoder()
            .audio()
            .context("Failed to create audio decoder")?;

        let resampler = ffmpeg::software::resampling::context::Context::get(
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed),
            ffmpeg::channel_layout::ChannelLayout::STEREO,
            44100,
        )?;

        Ok(Self { decoder, resampler })
    }

    pub fn sample_rate(&self) -> u32 {
        self.decoder.rate()
    }

    pub fn channels(&self) -> u16 {
        self.decoder.channels()
    }
}
