use crate::storage::OutputFormat;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct AudioExtractor;

impl AudioExtractor {
    pub fn extract(input: &Path, output: &Path, format: &OutputFormat) -> Result<()> {
        let output_str = output.to_string_lossy().into_owned();
        let input_str = input.to_string_lossy().into_owned();

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-i", &input_str, "-vn"]);

        match format {
            OutputFormat::Flac => {
                cmd.args(["-c:a", "flac"]);
            }
            OutputFormat::Mp3 { bitrate } => {
                cmd.args(["-c:a", "libmp3lame", "-b:a", &format!("{}k", bitrate)]);
            }
            OutputFormat::Opus { bitrate } => {
                cmd.args(["-c:a", "libopus", "-b:a", &format!("{}k", bitrate)]);
            }
        }

        cmd.arg("-y").arg(&output_str);

        let status = cmd.status().context("Failed to run ffmpeg")?;

        if !status.success() {
            anyhow::bail!("FFmpeg extraction failed with status: {}", status);
        }

        Ok(())
    }
}
