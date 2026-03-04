use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;
use std::time::Duration;

pub struct AudioDecoder {
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    input: ffmpeg::format::context::Input,
    stream_index: usize,
    output_sample_rate: u32,
    #[allow(dead_code)]
    playback_speed: f32,
    filter_graph: Option<ffmpeg::filter::Graph>,
}

impl AudioDecoder {
    pub fn from_url(url: &str) -> Result<Self> {
        Self::from_url_with_sample_rate(url, 44100)
    }

    pub fn from_url_with_sample_rate(url: &str, output_sample_rate: u32) -> Result<Self> {
        Self::from_url_with_sample_rate_and_speed(url, output_sample_rate, 1.0)
    }

    pub fn from_url_with_sample_rate_and_speed(
        url: &str,
        output_sample_rate: u32,
        speed: f32,
    ) -> Result<Self> {
        let speed = speed.clamp(0.5, 2.0);
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

        let filter_graph = if (speed - 1.0).abs() > f32::EPSILON {
            Some(build_atempo_filter_graph(&decoder, speed)?)
        } else {
            None
        };

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
            playback_speed: speed,
            filter_graph,
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

    pub fn duration(&self) -> Duration {
        let duration_micros = self.input.duration();
        if duration_micros > 0 {
            Duration::from_micros(duration_micros as u64)
        } else {
            Duration::ZERO
        }
    }

    pub fn decode_next(&mut self) -> Result<Option<Vec<i16>>> {
        if self.filter_graph.is_some() {
            self.decode_next_with_filter()
        } else {
            self.decode_next_simple()
        }
    }

    fn decode_next_simple(&mut self) -> Result<Option<Vec<i16>>> {
        for (stream, packet) in self.input.packets() {
            if stream.index() == self.stream_index {
                self.decoder.send_packet(&packet)?;

                let mut decoded = ffmpeg::frame::Audio::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    let samples = resample_and_collect(&mut self.resampler, &decoded)?;
                    if !samples.is_empty() {
                        return Ok(Some(samples));
                    }
                }
            }
        }

        self.decoder.send_eof()?;
        let mut decoded = ffmpeg::frame::Audio::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            let samples = resample_and_collect(&mut self.resampler, &decoded)?;
            if !samples.is_empty() {
                return Ok(Some(samples));
            }
        }

        Ok(None)
    }

    fn decode_next_with_filter(&mut self) -> Result<Option<Vec<i16>>> {
        let filter_graph = self
            .filter_graph
            .as_mut()
            .context("Filter graph not initialized")?;

        let mut src = filter_graph
            .get("in")
            .context("Failed to get filter source")?;
        let mut sink = filter_graph
            .get("out")
            .context("Failed to get filter sink")?;

        for (stream, packet) in self.input.packets() {
            if stream.index() == self.stream_index {
                self.decoder.send_packet(&packet)?;

                let mut decoded = ffmpeg::frame::Audio::empty();
                while self.decoder.receive_frame(&mut decoded).is_ok() {
                    src.source().add(&decoded)?;

                    let mut filtered = ffmpeg::frame::Audio::empty();
                    while sink.sink().frame(&mut filtered).is_ok() {
                        let samples = resample_and_collect(&mut self.resampler, &filtered)?;
                        if !samples.is_empty() {
                            return Ok(Some(samples));
                        }
                    }
                }
            }
        }

        self.decoder.send_eof()?;
        let mut decoded = ffmpeg::frame::Audio::empty();
        while self.decoder.receive_frame(&mut decoded).is_ok() {
            src.source().add(&decoded)?;

            let mut filtered = ffmpeg::frame::Audio::empty();
            while sink.sink().frame(&mut filtered).is_ok() {
                let samples = resample_and_collect(&mut self.resampler, &filtered)?;
                if !samples.is_empty() {
                    return Ok(Some(samples));
                }
            }
        }

        src.source().close(0)?;
        let mut filtered = ffmpeg::frame::Audio::empty();
        while sink.sink().frame(&mut filtered).is_ok() {
            let samples = resample_and_collect(&mut self.resampler, &filtered)?;
            if !samples.is_empty() {
                return Ok(Some(samples));
            }
        }

        Ok(None)
    }
}

fn extract_samples(frame: &ffmpeg::frame::Audio) -> Vec<i16> {
    let data = frame.data(0);
    let valid_bytes = frame.samples() * frame.channels() as usize * 2;
    let valid_data = &data[..valid_bytes.min(data.len())];
    valid_data
        .chunks_exact(2)
        .map(|chunk| i16::from_ne_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn resample_and_collect(
    resampler: &mut ffmpeg::software::resampling::Context,
    decoded: &ffmpeg::frame::Audio,
) -> Result<Vec<i16>> {
    let mut all_samples = Vec::new();

    let mut resampled = ffmpeg::frame::Audio::empty();
    let delay = resampler.run(decoded, &mut resampled)?;
    all_samples.extend(extract_samples(&resampled));

    if delay.is_some() {
        loop {
            let mut flushed = ffmpeg::frame::Audio::empty();
            match resampler.flush(&mut flushed)? {
                Some(_) => all_samples.extend(extract_samples(&flushed)),
                None => break,
            }
        }
    }

    Ok(all_samples)
}

/// Build a filter graph: abuffer → atempo → aformat → abuffersink
///
/// The `aformat` filter normalizes the output back to the decoder's native
/// sample format. This is needed because `atempo` may change the sample type
/// (e.g. from planar float to packed float), which would cause a mismatch
/// with the downstream resampler.
fn build_atempo_filter_graph(
    decoder: &ffmpeg::decoder::Audio,
    speed: f32,
) -> Result<ffmpeg::filter::Graph> {
    let mut graph = ffmpeg::filter::Graph::new();

    let sample_fmt = decoder.format();
    let sample_rate = decoder.rate();
    let channel_layout = decoder.channel_layout();

    let in_args = format!(
        "time_base=1/{}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
        sample_rate,
        sample_rate,
        sample_fmt.name(),
        channel_layout.bits()
    );

    let aformat_args = format!(
        "sample_fmts={}:sample_rates={}:channel_layouts=0x{:x}",
        sample_fmt.name(),
        sample_rate,
        channel_layout.bits()
    );

    let abuffer = ffmpeg::filter::find("abuffer").context("Could not find abuffer filter")?;
    let atempo = ffmpeg::filter::find("atempo").context("Could not find atempo filter")?;
    let aformat = ffmpeg::filter::find("aformat").context("Could not find aformat filter")?;
    let abuffersink =
        ffmpeg::filter::find("abuffersink").context("Could not find abuffersink filter")?;

    let mut ctx_in = graph.add(&abuffer, "in", &in_args)?;
    let mut ctx_atempo = graph.add(&atempo, "atempo", &format!("tempo={}", speed))?;
    let mut ctx_aformat = graph.add(&aformat, "aformat", &aformat_args)?;
    let mut ctx_out = graph.add(&abuffersink, "out", "")?;

    ctx_in.link(0, &mut ctx_atempo, 0);
    ctx_atempo.link(0, &mut ctx_aformat, 0);
    ctx_aformat.link(0, &mut ctx_out, 0);

    graph
        .validate()
        .context("Failed to validate filter graph")?;

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atempo_filter_graph_builds_and_validates() {
        ffmpeg::init().unwrap();

        let mut graph = ffmpeg::filter::Graph::new();

        let abuffer = ffmpeg::filter::find("abuffer").unwrap();
        let atempo = ffmpeg::filter::find("atempo").unwrap();
        let aformat = ffmpeg::filter::find("aformat").unwrap();
        let abuffersink = ffmpeg::filter::find("abuffersink").unwrap();

        let in_args = "time_base=1/44100:sample_rate=44100:sample_fmt=fltp:channel_layout=0x3";
        let aformat_args = "sample_fmts=fltp:sample_rates=44100:channel_layouts=0x3";

        let mut ctx_in = graph.add(&abuffer, "in", in_args).expect("add abuffer");
        let mut ctx_atempo = graph
            .add(&atempo, "atempo", "tempo=1.5")
            .expect("add atempo");
        let mut ctx_aformat = graph
            .add(&aformat, "aformat", aformat_args)
            .expect("add aformat");
        let mut ctx_out = graph.add(&abuffersink, "out", "").expect("add abuffersink");

        ctx_in.link(0, &mut ctx_atempo, 0);
        ctx_atempo.link(0, &mut ctx_aformat, 0);
        ctx_aformat.link(0, &mut ctx_out, 0);

        graph.validate().expect("validate filter graph");
    }
}
