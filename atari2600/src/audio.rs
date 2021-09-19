//! An audio player module. Note that currently, it's based on Rodio, because
//! Rodio is easy to use. Unfortunately, Rodio doesn't have a good resampling
//! algorithm, and since Atari generates audio with 31kHz sampling rate, this
//! influences the sound quality. Let's revisit this in future.

use rodio::OutputStream;
use rodio::Sink;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

pub struct AudioConsumer {
    sender: SyncSender<f32>,
}

impl AudioConsumer {
    pub fn consume(&self, sample: f32) {
        if let Err(e) = self.sender.send(sample) {
            eprintln!("Unable to send audio sample: {}", e);
        }
    }
}

pub struct AudioSource {
    receiver: Receiver<f32>,
}

impl rodio::Source for AudioSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        31440
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Iterator for AudioSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver
            .recv()
            .map_err(|e| {
                eprintln!("Unable to retrieve audio sample: {}", e);
                e
            })
            .ok()
    }
}

pub fn create_consumer_and_source() -> (AudioConsumer, AudioSource) {
    let (sender, receiver) = sync_channel(10000);
    (AudioConsumer { sender }, AudioSource { receiver })
}

pub fn initialize() -> (AudioConsumer, OutputStream, Sink) {
    let (stream, stream_handle) = OutputStream::try_default().unwrap();
    let audio_sink = Sink::try_new(&stream_handle).unwrap();
    audio_sink.set_volume(0.1);
    let (audio_consumer, audio_source) = create_consumer_and_source();
    audio_sink.append(audio_source);
    return (audio_consumer, stream, audio_sink);
}
