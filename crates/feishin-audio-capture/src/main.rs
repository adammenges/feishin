#[cfg(any(target_os = "macos", test))]
#[path = "../../feishin-core/src/system_audio_pcm.rs"]
mod pcm;

#[cfg(target_os = "macos")]
fn capture() -> anyhow::Result<()> {
    use anyhow::Context;
    use screencapturekit::prelude::*;
    use screencapturekit::stream::delegate_trait::StreamCallbacks;
    use std::{
        io::Write,
        sync::{Arc, Mutex, mpsc},
    };
    // This helper's macOS 13 runtime is isolated from the macOS 12 player.
    let content = SCShareableContent::get()
        .context("System audio requires macOS screen recording permission")?;
    let display = content
        .displays()
        .into_iter()
        .next()
        .context("No display is available for system audio")?;
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build()?;
    let config = SCStreamConfiguration::new()
        .with_width(2)
        .with_height(2)
        .with_captures_audio(true)
        .with_sample_rate(48000)
        .with_channel_count(2)
        .with_excludes_current_process_audio(false);
    let (errors, failure) = mpsc::sync_channel(8);
    let delegate_errors = errors.clone();
    let delegate = StreamCallbacks::new().on_error(move |error| {
        let _ = delegate_errors.try_send(error.to_string());
    });
    let mut stream = SCStream::new_with_delegate(&filter, &config, delegate)?;
    let stdout = Arc::new(Mutex::new(std::io::stdout()));
    stream.add_output_handler(
        move |sample: CMSampleBuffer, kind: SCStreamOutputType| {
            if kind != SCStreamOutputType::Audio {
                return;
            }
            let result = (|| -> anyhow::Result<()> {
                let format = sample
                    .format_description()
                    .context("Missing system-audio format")?;
                anyhow::ensure!(
                    format.audio_is_float() && format.audio_bits_per_channel() == Some(32),
                    "Unexpected system-audio sample format"
                );
                let buffers = sample
                    .audio_buffer_list()
                    .map_err(|code| anyhow::anyhow!("Could not read system audio: {code}"))?;
                let buffers = buffers
                    .iter()
                    .map(|buffer| buffer.data())
                    .collect::<Vec<_>>();
                let data = pcm::stereo_f32(&buffers, format.audio_is_big_endian())?;
                let bytes = data
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>();
                stdout
                    .lock()
                    .map_err(|_| anyhow::anyhow!("System-audio output failed"))?
                    .write_all(&bytes)?;
                Ok(())
            })();
            if let Err(error) = result {
                let _ = errors.try_send(error.to_string());
            }
        },
        SCStreamOutputType::Audio,
    )?;
    stream.start_capture()?;
    let (stop, stopped) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0_u8; 1]);
        let _ = stop.send(());
    });
    loop {
        if let Ok(error) = failure.try_recv() {
            anyhow::bail!("{error}");
        }
        if stopped.recv_timeout(std::time::Duration::from_millis(100))
            != Err(mpsc::RecvTimeoutError::Timeout)
        {
            break;
        }
    }
    stream.stop_capture()?;
    Ok(())
}

fn main() {
    if std::env::args().nth(1).as_deref() != Some("--capture") {
        eprintln!("Feishin's system-audio helper starts from the visualizer capture control.");
        return;
    }
    #[cfg(target_os = "macos")]
    if let Err(error) = capture() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("This helper is used only for macOS system audio.");
        std::process::exit(1);
    }
}
