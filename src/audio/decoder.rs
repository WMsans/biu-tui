use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;

pub struct AudioDecoder {
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    input: ffmpeg::format::context::Input,
    stream_index: usize,
    output_sample_rate: u32,
}

impl AudioDecoder {
    pub fn from_url(url: &str) -> Result<Self> {
        Self::from_url_with_sample_rate(url, 44100)
    }

    pub fn from_url_with_sample_rate(url: &str, output_sample_rate: u32) -> Result<Self> {
        ffmpeg::init()?;

        let mut options = ffmpeg::Dictionary::new();
        options.set("headers", "Referer: https://www.bilibili.com\r\nUser-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36\r\n");

        let ictx = ffmpeg::format::input_with_dictionary(&url, options)
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
            output_sample_rate,
        )?;

        Ok(Self {
            decoder,
            resampler,
            input: ictx,
            stream_index,
            output_sample_rate,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.decoder.rate()
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.output_sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.decoder.channels()
    }

    pub fn decode_next(&mut self) -> Result<Option<Vec<i16>>> {
        for (stream, packet) in self.input.packets() {
            if stream.index() == self.stream_index {
                self.decoder.send_packet(&packet)?;

                let mut decoded = ffmpeg::frame::Audio::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    let mut resampled = ffmpeg::frame::Audio::empty();
                    self.resampler.run(&decoded, &mut resampled)?;

                    let data = resampled.data(0);
                    let samples: Vec<i16> = data
                        .chunks_exact(2)
                        .map(|chunk| i16::from_ne_bytes([chunk[0], chunk[1]]))
                        .collect();

                    return Ok(Some(samples));
                }
            }
        }

        self.decoder.send_eof()?;
        let mut decoded = ffmpeg::frame::Audio::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            let mut resampled = ffmpeg::frame::Audio::empty();
            self.resampler.run(&decoded, &mut resampled)?;

            let data = resampled.data(0);
            let samples: Vec<i16> = data
                .chunks_exact(2)
                .map(|chunk| i16::from_ne_bytes([chunk[0], chunk[1]]))
                .collect();

            return Ok(Some(samples));
        }

        Ok(None)
    }
}
