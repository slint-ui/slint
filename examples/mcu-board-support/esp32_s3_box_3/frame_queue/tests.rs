// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::*;
use slint::platform::software_renderer::{Position, Size};
use std::{
    sync::{Arc, Barrier, mpsc},
    thread,
    time::Duration,
};

fn queue(capacity: usize) -> FrameQueue {
    FrameQueue::new(Box::leak(vec![0; capacity].into_boxed_slice()))
}

fn pixels() -> Vec<Rgb565BigEndianPixel> {
    (0..320 * 240).map(|index| Rgb565BigEndianPixel((index as u16).to_be())).collect()
}

fn region(rectangles: &[(i32, i32, u32, u32)]) -> PhysicalRegion {
    PhysicalRegion(
        rectangles
            .iter()
            .map(|&(x, y, width, height)| (Position { x, y }, Size { width, height }))
            .collect(),
    )
}

fn acquire(queue: &FrameQueue) -> &Packet {
    assert_eq!(
        queue.state.compare_exchange(READY, ACTIVE, Ordering::Acquire, Ordering::Relaxed),
        Ok(READY)
    );
    unsafe { &*queue.packet.get() }
}

fn bytes(packet: &Packet) -> &[u8] {
    if packet.internal_used { packet.internal } else { &packet.external }
}

#[test]
fn disjoint_rectangles_preserve_wire_bytes_and_offsets() {
    for capacity in [0, 64 * 1024] {
        let queue = queue(capacity);
        let mut source = pixels();
        queue.publish(&source, &region(&[(3, 7, 5, 2), (300, 230, 20, 10)]));
        source.fill(Rgb565BigEndianPixel(0));
        let packet = acquire(&queue);
        assert_eq!(packet.count, 2);
        assert_eq!(packet.internal_used, capacity != 0);
        #[cfg(feature = "box-profile")]
        assert_eq!(packet.ready_us, 42);
        let output = bytes(packet);
        let mut offset = 0;
        for (rect, &(x, y, width, height)) in
            packet.rectangles[..packet.count].iter().zip(&[(3, 7, 5, 2), (300, 230, 20, 10)])
        {
            assert_eq!(
                (rect.x, rect.y, rect.width, rect.height, rect.offset),
                (x, y, width, height, offset)
            );
            for row in 0..height {
                for column in 0..width {
                    let expected = (((y + row) * 320 + x + column) as u16).to_be_bytes();
                    assert_eq!(&output[offset..offset + 2], &expected);
                    offset += 2;
                }
            }
        }
    }
}

#[test]
fn buffer_selection_includes_exact_capacity_and_full_frame_fallback() {
    for (capacity, full, expected_internal) in
        [(16, false, true), (15, false, false), (65536, true, false)]
    {
        let queue = queue(capacity);
        let (width, height) = if full { (320, 240) } else { (4, 2) };
        queue.publish(&pixels(), &region(&[(0, 0, width, height)]));
        let packet = acquire(&queue);
        assert_eq!(packet.internal_used, expected_internal);
        let output = bytes(packet);
        for row in 0..height as usize {
            for column in 0..width as usize {
                let offset = (row * width as usize + column) * 2;
                assert_eq!(
                    &output[offset..offset + 2],
                    &((row * 320 + column) as u16).to_be_bytes()
                );
            }
        }
    }
}

#[test]
fn empty_region_does_not_publish_a_packet() {
    let queue = queue(64);
    queue.publish(&pixels(), &PhysicalRegion::default());
    assert_eq!(queue.state.load(Ordering::Acquire), FREE);
    assert_eq!(unsafe { &*queue.packet.get() }.count, 0);
}

#[test]
fn excessive_rectangles_pack_the_entire_bounding_box() {
    let queue = queue(64 * 1024);
    let rectangles: Vec<_> = (0..MAX_RECTS as i32 + 1).map(|x| (x * 2, 10, 1, 1)).collect();
    queue.publish(&pixels(), &region(&rectangles));
    let packet = acquire(&queue);
    assert_eq!(packet.count, 1);
    let rectangle = packet.rectangles[0];
    assert_eq!(
        (rectangle.x, rectangle.y, rectangle.width, rectangle.height, rectangle.offset),
        (0, 10, 129, 1, 0)
    );
    for x in 0..129 {
        assert_eq!(&bytes(packet)[x * 2..x * 2 + 2], &((3200 + x) as u16).to_be_bytes());
    }
}

#[test]
fn producer_waits_for_consumer_completion_in_ready_and_active_states() {
    let queue = Arc::new(queue(64));
    queue.publish(&pixels(), &region(&[(3, 7, 5, 2)]));
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let producer_queue = queue.clone();
    let producer = thread::spawn(move || {
        let source = vec![Rgb565BigEndianPixel(0x1234u16.to_be()); 320 * 240];
        started_tx.send(()).unwrap();
        producer_queue.publish(&source, &region(&[(3, 7, 5, 2)]));
        done_tx.send(()).unwrap();
    });
    started_rx.recv().unwrap();
    assert_eq!(
        done_rx.recv_timeout(Duration::from_millis(10)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    let original = bytes(acquire(&queue))[..20].to_vec();
    assert_eq!(
        done_rx.recv_timeout(Duration::from_millis(10)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    assert_eq!(&bytes(unsafe { &*queue.packet.get() })[..20], original.as_slice());
    queue.state.store(FREE, Ordering::Release);
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    producer.join().unwrap();
    let packet = acquire(&queue);
    assert_eq!(&bytes(packet)[..20], &[0x12, 0x34].repeat(10));
}

#[test]
fn concurrent_publishers_reserve_exclusive_packet_access() {
    let queue = Arc::new(queue(64 * 1024));
    let start = Arc::new(Barrier::new(3));
    let producers: Vec<_> = [0x125au16, 0xe391u16]
        .into_iter()
        .map(|value| {
            let queue = queue.clone();
            let start = start.clone();
            thread::spawn(move || {
                let source = vec![Rgb565BigEndianPixel(value.to_be()); 320 * 240];
                start.wait();
                for _ in 0..40 {
                    queue.publish(&source, &region(&[(0, 0, 320, 80)]));
                }
            })
        })
        .collect();
    start.wait();
    let mut counts = [0; 2];
    let mut all_packets_uniform = true;
    for _ in 0..80 {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while queue
            .state
            .compare_exchange(READY, ACTIVE, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            assert!(std::time::Instant::now() < deadline, "publication timed out");
            thread::yield_now();
        }
        let packet = unsafe { &*queue.packet.get() };
        let output = &bytes(packet)[..320 * 80 * 2];
        let first = [output[0], output[1]];
        match first {
            [0x12, 0x5a] => counts[0] += 1,
            [0xe3, 0x91] => counts[1] += 1,
            _ => all_packets_uniform = false,
        }
        all_packets_uniform &= output.chunks_exact(2).all(|pixel| pixel == first);
        all_packets_uniform &= packet.count == 1 && packet.rectangles[0].height == 80;
        queue.state.store(FREE, Ordering::Release);
    }
    for producer in producers {
        producer.join().unwrap();
    }
    assert!(all_packets_uniform);
    assert_eq!(counts, [40, 40]);
}

#[cfg(feature = "box-profile")]
#[test]
fn packet_target_is_published_with_immutable_metadata_and_wraps() {
    let queue = queue(65536);
    queue.render_tick.store(u32::MAX, Ordering::Release);
    queue.publish(&pixels(), &region(&[(0, 0, 1, 1)]));
    let packet = acquire(&queue);
    assert_eq!(packet.target_tick, 0);
    assert_eq!(packet_late_cycles(0, 0), 0);
    assert_eq!(packet_late_cycles(1, 0), 1);
    assert_eq!(packet_late_cycles(0, u32::MAX), 1);
    assert_eq!(packet_late_cycles(u32::MAX, 0), 0);
}

#[cfg(feature = "box-line-damage")]
#[test]
fn scanline_damage_tracks_previous_queued_frame_without_an_external_shadow() {
    let queue = queue(65536);
    let mut pixels = vec![Rgb565BigEndianPixel(0); 320 * 240];
    let mut lcd = vec![0u16; 320 * 240];
    for frame in 0..300 {
        let mut plan = super::pixel_damage::Plan::new();
        for y in 0..240 {
            let mut row = [0u16; 320];
            for x in 4..310 {
                if (x + frame) % 41 < 7 && (y + frame) % 29 < 13 {
                    row[x] = (frame + x * 23 + y * 47) as u16;
                }
            }
            let previous: Vec<_> =
                pixels[y * 320..(y + 1) * 320].iter().map(|pixel| pixel.0).collect();
            if let Some(span) = super::pixel_damage::changed_line(&row, &previous, 0, y) {
                let bounds = span.copy_bounds();
                plan.add(span);
                for (left, right) in bounds {
                    if left < right {
                        for (pixel, value) in pixels[y * 320 + left..y * 320 + right]
                            .iter_mut()
                            .zip(&row[left..right])
                        {
                            pixel.0 = *value;
                        }
                    }
                }
            }
            assert!(
                pixels[y * 320..(y + 1) * 320]
                    .iter()
                    .zip(row)
                    .all(|(pixel, value)| pixel.0 == value)
            );
        }
        queue.publish_rectangles(&pixels, plan.rectangles());
        if queue.state.load(Ordering::Acquire) != FREE {
            assert_eq!(queue.state.swap(ACTIVE, Ordering::Acquire), READY);
            let packet = unsafe { &*queue.packet.get() };
            let source =
                if packet.internal_used { &packet.internal[..] } else { &packet.external[..] };
            for rect in &packet.rectangles[..packet.count] {
                for y in 0..rect.height {
                    for x in 0..rect.width {
                        let byte = rect.offset + (y * rect.width + x) * 2;
                        lcd[(rect.y + y) * 320 + rect.x + x] =
                            u16::from_ne_bytes([source[byte], source[byte + 1]]);
                    }
                }
            }
            queue.state.store(FREE, Ordering::Release);
        }
        assert!(lcd.iter().zip(&pixels).all(|(a, b)| *a == b.0));
    }
}

#[cfg(feature = "box-line-damage")]
#[test]
fn overlapping_damage_cover_falls_back_before_exceeding_packet_capacity() {
    let queue = queue(65536);
    let pixels = vec![Rgb565BigEndianPixel(0x1234); 320 * 240];
    let rectangles = [super::pixel_damage::Rect { x: 0, y: 0, width: 320, height: 240 }; 8];
    queue.publish_rectangles(&pixels, &rectangles);
    let packet = acquire(&queue);
    assert_eq!(packet.count, 1);
    assert_eq!((packet.rectangles[0].width, packet.rectangles[0].height), (320, 240));
    assert!(!packet.internal_used);
    assert!(packet.external.chunks_exact(2).all(|bytes| bytes == 0x1234u16.to_ne_bytes()));
}

#[cfg(all(feature = "box-line-damage", feature = "box-profile"))]
#[test]
fn exact_damage_proves_a_changed_pixel_missed_by_sampled_hash() {
    let queue = queue(65536);
    let mut pixels = vec![Rgb565BigEndianPixel(0); 320 * 240];
    queue.publish(&pixels, &region(&[(0, 0, 320, 240)]));
    let old_hash = acquire(&queue).hash;
    queue.state.store(FREE, Ordering::Release);
    let previous = [0u16; 320];
    let mut current = previous;
    current[1] = 0xffff;
    pixels[1].0 = 0xffff;
    let mut plan = super::pixel_damage::Plan::new();
    plan.line(&current, &previous, 0, 0);
    queue.publish_damage(&pixels, &plan);
    let packet = acquire(&queue);
    assert!(packet.verified_change);
    let same_sampled_hash = pixels
        .iter()
        .step_by(8)
        .fold(2166136261u32, |hash, pixel| (hash ^ pixel.0 as u32).wrapping_mul(16777619));
    assert_eq!(old_hash, same_sampled_hash);
    assert!(super::packet_changes_pixels(packet, &mut Some(old_hash)));
    queue.state.store(FREE, Ordering::Release);
    let mut unchanged = super::pixel_damage::Plan::new();
    unchanged.line(&current, &current, 0, 0);
    queue.publish_damage(&pixels, &unchanged);
    assert_eq!(queue.state.load(Ordering::Acquire), FREE);
}

#[cfg(feature = "box-line-damage")]
#[test]
fn empty_exact_plan_preserves_an_in_flight_packet_without_waiting() {
    for state in [READY, ACTIVE, WRITING] {
        let queue = Arc::new(queue(64));
        queue.publish(&pixels(), &region(&[(3, 7, 5, 2)]));
        queue.state.store(state, Ordering::Release);
        let previous_bytes = bytes(unsafe { &*queue.packet.get() })[..20].to_vec();
        let (sent, received) = mpsc::channel();
        let producer_queue = queue.clone();
        let producer = thread::spawn(move || {
            producer_queue.publish_damage(&pixels(), &super::pixel_damage::Plan::new());
            sent.send(()).unwrap();
        });
        let completed = received.recv_timeout(Duration::from_secs(1)).is_ok();
        let after = queue.state.load(Ordering::Acquire);
        queue.state.store(FREE, Ordering::Release);
        producer.join().unwrap();
        let after_bytes = bytes(unsafe { &*queue.packet.get() })[..20].to_vec();
        assert!(completed, "an empty plan waited for an in-flight packet");
        assert_eq!(after, state);
        assert_eq!(after_bytes, previous_bytes);
    }
}
