// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use futures::{future::OptionFuture, FutureExt};

use super::ControlCommand;

pub struct VideoPlaybackThread {
    control_sender: smol::channel::Sender<ControlCommand>,
    packet_sender: smol::channel::Sender<ffmpeg_next::codec::packet::packet::Packet>,
    receiver_thread: Option<std::thread::JoinHandle<()>>,
}

impl VideoPlaybackThread {
    pub fn start(
        stream: &ffmpeg_next::format::stream::Stream,
        mut video_frame_callback: Box<dyn FnMut(&ffmpeg_next::util::frame::Video) + Send>,
    ) -> Result<Self, anyhow::Error> {
        let (control_sender, control_receiver) = smol::channel::unbounded();

        let (packet_sender, packet_receiver) = smol::channel::bounded(128);

        let decoder_context = ffmpeg_next::codec::Context::from_parameters(stream.parameters())?;
        let mut packet_decoder = decoder_context.decoder().video()?;

        let clock = StreamClock::new(stream);

        let receiver_thread =
            std::thread::Builder::new().name("video playback thread".into()).spawn(move || {
                smol::block_on(async move {
                    let packet_receiver_impl = async {
                        loop {
                            let Ok(packet) = packet_receiver.recv().await else { break };

                            smol::future::yield_now().await;

                            packet_decoder.send_packet(&packet).unwrap();

                            let mut decoded_frame = ffmpeg_next::util::frame::Video::empty();

                            while packet_decoder.receive_frame(&mut decoded_frame).is_ok() {
                                if let Some(delay) =
                                    clock.convert_pts_to_instant(decoded_frame.pts())
                                {
                                    smol::Timer::after(delay).await;
                                }

                                video_frame_callback(&decoded_frame);
                            }
                        }
                    }
                    .fuse()
                    .shared();

                    let mut playing = true;

                    loop {
                        let packet_receiver: OptionFuture<_> =
                            if playing { Some(packet_receiver_impl.clone()) } else { None }.into();

                        smol::pin!(packet_receiver);

                        futures::select! {
                            _ = packet_receiver => {},
                            received_command = control_receiver.recv().fuse() => {
                                match received_command {
                                    Ok(ControlCommand::Pause) => {
                                        playing = false;
                                    }
                                    Ok(ControlCommand::Play) => {
                                        playing = true;
                                    }
                                    Err(_) => {
                                        // Channel closed -> quit
                                        return;
                                    }
                                }
                            }
                        }
                    }
                })
            })?;

        Ok(Self { control_sender, packet_sender, receiver_thread: Some(receiver_thread) })
    }

    pub async fn receive_packet(&self, packet: ffmpeg_next::codec::packet::packet::Packet) -> bool {
        match self.packet_sender.send(packet).await {
            Ok(_) => return true,
            Err(smol::channel::SendError(_)) => return false,
        }
    }

    pub async fn send_control_message(&self, message: ControlCommand) {
        self.control_sender.send(message).await.unwrap();
    }
}

impl Drop for VideoPlaybackThread {
    fn drop(&mut self) {
        self.control_sender.close();
        if let Some(receiver_join_handle) = self.receiver_thread.take() {
            receiver_join_handle.join().unwrap();
        }
    }
}

struct StreamClock {
    time_base_seconds: f64,
    start_time: std::time::Instant,
}

impl StreamClock {
    fn new(stream: &ffmpeg_next::format::stream::Stream) -> Self {
        let time_base_seconds = stream.time_base();
        let time_base_seconds =
            time_base_seconds.numerator() as f64 / time_base_seconds.denominator() as f64;

        let start_time = std::time::Instant::now();

        Self { time_base_seconds, start_time }
    }

    fn convert_pts_to_instant(&self, pts: Option<i64>) -> Option<std::time::Duration> {
        pts.and_then(|pts| {
            let pts_since_start =
                std::time::Duration::from_secs_f64(pts as f64 * self.time_base_seconds);
            self.start_time.checked_add(pts_since_start)
        })
        .map(|absolute_pts| absolute_pts.duration_since(std::time::Instant::now()))
    }
}
