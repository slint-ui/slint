// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore jtag csrr mhartid

// Records the spans of the runtime (from api/cpp/trace.rs) and of this component with as little
// work as possible, and sends them over the console only after the capture ends, so that
// neither recording nor sending disturbs the measured frames. `idf.py trace` (idf_ext.py)
// decodes them.
//
// The capture is a sequence of little-endian 32-bit words:
// - `id | kind`, `cycles`: a span enter (0), exit (1) or event (2). `id` is the address of the
//   call site's metadata, and `cycles` the low 32 bits of the CPU cycle counter.
// - `3 | type << 2 | payload_words << 8`, then the payload:
//   - type 0, a name: its address, byte length, and UTF-8 bytes padded to whole words.
//   - type 1, a field of the next span or event: name address, value low and high words.
//   - type 2, the clock: CPU cycles per microsecond.
//   - type 3, the measured cost of recording one empty span, in cycles.
//   - type 4, a span on another track, such as hardware working in the background: its name
//     address, begin and end cycles, and the track number.
//   - type 5, the start of the records of another CPU core: its track number. Its cycles are
//     converted to those of the first core.
//
// Each packet is `\x1eB`, the escaped sequence number and capture bytes, and `\n`. `\x1eE\n`
// ends the capture. Escaping keeps `\n` and `\r` out of packets, so they pass the console.

#include "slint-esp-trace.h"

#ifdef SLINT_ESP_TRACING

#    include <algorithm>
#    include <cstdio>
#    include <cstdlib>
#    include <cstring>
#    include "esp_cpu.h"
#    include "esp_heap_caps.h"
#    include "esp_rom_sys.h"
#    include "esp_timer.h"
#    if __has_include("soc/soc_caps.h")
#        include "soc/soc_caps.h"
#    endif
#    if SOC_USB_SERIAL_JTAG_SUPPORTED
#        include "driver/usb_serial_jtag.h"
#        include "driver/usb_serial_jtag_vfs.h"
#    endif

namespace {

// Skips the start-up before recording.
constexpr uint32_t DELAY_US = 2'000'000;
constexpr uint32_t DURATION_US = 5'000'000;
// The capture takes a quarter of the largest free block, and leaves RESERVE_BYTES of it free.
constexpr size_t MAX_CAPTURE_BYTES = 16u << 20;
constexpr size_t MIN_CAPTURE_BYTES = 256u << 10;
constexpr size_t RESERVE_BYTES = 2u << 20;

constexpr uint32_t ENTER = 0;
constexpr uint32_t EXIT = 1;
constexpr uint32_t EVENT = 2;
constexpr uint32_t EXTENDED = 3;
constexpr uint32_t NAME = 0;
constexpr uint32_t FIELD = 1;
constexpr uint32_t CLOCK = 2;
constexpr uint32_t SPAN_COST = 3;
constexpr uint32_t CORE = 5;

// Each core records into its own part of the capture, so that recording needs no locking.
constexpr size_t CORES = 2;
// The Tracy track of each core's records. Track 1 is the panel transfer.
constexpr uint32_t CORE_TRACKS[CORES] = { 0, 2 };

constexpr auto relaxed = std::memory_order_relaxed;

std::atomic<uint32_t *> capture[CORES];
std::atomic<size_t> capacity { 0 };
std::atomic<size_t> len[CORES];
// What to add to each core's cycle count to get the first core's.
std::atomic<uint32_t> cycle_offset[CORES];
std::atomic<bool> aligned[CORES];
std::atomic<bool> recording { false };
std::atomic<uint32_t> installed { 0 };
std::atomic<uint32_t> delay { 0 };
std::atomic<uint32_t> end { UINT32_MAX };
// The time and the first core's cycle count at one moment, to align the other cores' with.
std::atomic<uint32_t> reference_us { 0 };
std::atomic<uint32_t> reference_cycles { 0 };
std::atomic<uint32_t> ticks_per_us { 0 };
size_t probe_len = 0;

// The ESP32-P4, the only RISC-V ESP with an FPU, has the standard cycle counter.
#    if defined(__riscv) && defined(__riscv_flen)
inline size_t core_id()
{
    size_t hart;
    asm volatile("csrr %0, mhartid" : "=r"(hart));
    return std::min(hart, CORES - 1);
}

inline uint32_t core_cycles()
{
    uint32_t cycles;
    asm volatile("csrr %0, cycle" : "=r"(cycles));
    return cycles;
}
#    else
inline size_t core_id()
{
    return std::min<size_t>(esp_cpu_get_core_id(), CORES - 1);
}

inline uint32_t core_cycles()
{
    return esp_cpu_get_cycle_count();
}
#    endif

// Sets what `core`'s cycle count needs to match the first core's, once the capture has started.
void align_core(size_t core)
{
    if (reference_us.load(relaxed) == 0) {
        return;
    }
    const uint32_t elapsed_us = uint32_t(esp_timer_get_time()) - reference_us.load(relaxed);
    const uint32_t first_core_now =
            reference_cycles.load(relaxed) + elapsed_us * ticks_per_us.load(relaxed);
    cycle_offset[core].store(first_core_now - core_cycles(), relaxed);
    aligned[core].store(true, relaxed);
}

// The cycle count of the first core, wherever this runs.
// A task that isn't pinned can run on any core, so a core aligns itself when it first records.
inline uint32_t cycles()
{
    const size_t core = core_id();
    if (core != 0 && !aligned[core].load(relaxed)) {
        align_core(core);
    }
    return core_cycles() + cycle_offset[core].load(relaxed);
}

// Without the driver, the console drops output while the host isn't reading fast enough.
// The console switches to the driver as well, so its output doesn't bypass the driver's buffer.
// The driver allocates internal RAM, which a running application can use up, so the capture
// installs it when it starts.
void install_console_driver()
{
#    if SOC_USB_SERIAL_JTAG_SUPPORTED
    if (!usb_serial_jtag_is_driver_installed()) {
        usb_serial_jtag_driver_config_t config = {};
        config.tx_buffer_size = 8192;
        config.rx_buffer_size = 256;
        if (usb_serial_jtag_driver_install(&config) == ESP_OK) {
            usb_serial_jtag_vfs_use_driver();
        }
    }
#    endif
}

void write_all(const uint8_t *data, size_t length)
{
#    if SOC_USB_SERIAL_JTAG_SUPPORTED
    install_console_driver();
    if (usb_serial_jtag_is_driver_installed()) {
        while (length > 0) {
            const int written = usb_serial_jtag_write_bytes(data, length, portMAX_DELAY);
            if (written <= 0) {
                return;
            }
            data += written;
            length -= written;
        }
        return;
    }
#    endif
    fwrite(data, 1, length, stdout);
    fflush(stdout);
}

void append_escaped(uint8_t *packet, size_t &length, uint8_t byte)
{
    if (byte == '\n' || byte == '\r' || byte == 0x1b || byte == 0x1e) {
        packet[length++] = 0x1b;
        packet[length++] = byte ^ 0x40;
    } else {
        packet[length++] = byte;
    }
}

// Stops recording and sends the capture. Runs on the thread that recorded last, which blocks
// until everything is sent.
void finish()
{
    if (!recording.exchange(false, relaxed)) {
        return;
    }

    // Room for the prefix, an escaped sequence number and chunk, and the newline.
    uint8_t packet[1024];
    uint8_t chunk[480];
    size_t chunk_len = 0;
    uint32_t sequence = 0;
    auto send = [&packet](const uint8_t *bytes, size_t count, uint32_t sequence) {
        packet[0] = 0x1e;
        packet[1] = 'B';
        size_t length = 2;
        while (sequence >= 0x80) {
            append_escaped(packet, length, uint8_t(sequence) | 0x80);
            sequence >>= 7;
        }
        append_escaped(packet, length, uint8_t(sequence));
        for (size_t i = 0; i < count; ++i) {
            append_escaped(packet, length, bytes[i]);
        }
        packet[length++] = '\n';
        write_all(packet, length);
    };
    // Each core's records follow the previous core's. Another core can still be adding records
    // after its length, which are left out.
    for (size_t core = 0; core < CORES; ++core) {
        const auto *bytes = reinterpret_cast<const uint8_t *>(capture[core].load(relaxed));
        const size_t count = len[core].load(relaxed) * 4;
        for (size_t i = 0; i < count; ++i) {
            chunk[chunk_len++] = bytes[i];
            if (chunk_len == sizeof(chunk)) {
                send(chunk, chunk_len, sequence++);
                chunk_len = 0;
            }
        }
    }
    if (chunk_len > 0) {
        send(chunk, chunk_len, sequence);
    }
    write_all(reinterpret_cast<const uint8_t *>("\x1e" "E\n"), 3);
}

// Returns whether records are being kept now, and ends the capture when it's over.
inline bool recording_now(uint32_t &now)
{
    if (!recording.load(relaxed)) {
        return false;
    }
    now = cycles();
    const uint32_t elapsed = now - installed.load(relaxed);
    if (elapsed < delay.load(relaxed)) {
        return false;
    }
    if (elapsed >= end.load(relaxed)) {
        finish();
        return false;
    }
    return true;
}

inline void push(const uint32_t *words, size_t count)
{
    const size_t core = core_id();
    const size_t length = len[core].load(relaxed);
    if (length + count > capacity.load(relaxed)) {
        finish();
        return;
    }
    std::memcpy(capture[core].load(relaxed) + length, words, count * sizeof(uint32_t));
    len[core].store(length + count, relaxed);
}

inline void record(uint32_t id, uint32_t kind)
{
    uint32_t now;
    if (recording_now(now)) {
        const uint32_t words[] = { id | kind, now };
        push(words, 2);
    }
}

constexpr uint32_t extended(uint32_t kind, uint32_t payload_words)
{
    return EXTENDED | kind << 2 | payload_words << 8;
}

void push_name(uint32_t address, const char *name, size_t name_len)
{
    if (!recording.load(relaxed)) {
        return;
    }
    const size_t text_words = (name_len + 3) / 4;
    const size_t core = core_id();
    if (len[core].load(relaxed) + 3 + text_words > capacity.load(relaxed)) {
        finish();
        return;
    }
    const uint32_t header[] = { extended(NAME, 2 + text_words), address, uint32_t(name_len) };
    push(header, 3);
    const size_t start = len[core].load(relaxed);
    uint32_t *words = capture[core].load(relaxed) + start;
    std::memset(words, 0, text_words * sizeof(uint32_t));
    std::memcpy(words, name, name_len);
    len[core].store(start + text_words, relaxed);
}

void push_field(uint32_t name, uint64_t value)
{
    uint32_t now;
    if (recording_now(now)) {
        const uint32_t words[] = { extended(FIELD, 3), name, uint32_t(value),
                                   uint32_t(value >> 32) };
        push(words, 4);
    }
}

uint32_t site_id(slint_trace::Site *site)
{
    const auto id = uint32_t(uintptr_t(site));
    if (recording.load(relaxed) && !site->registered.exchange(true, relaxed)) {
        push_name(id, site->name, std::strlen(site->name));
    }
    return id;
}

}

namespace slint_trace {

void enter(Site *site)
{
    record(site_id(site), ENTER);
}

void exit(Site *site)
{
    record(site_id(site), EXIT);
}

void field(Site *site, int64_t value)
{
    push_field(site_id(site), uint64_t(value));
}

void register_core()
{
    const size_t core = core_id();
    if (core != 0) {
        align_core(core);
    }
}

}

// The functions that api/cpp/trace.rs calls for the spans of the runtime.
extern "C" {

// Starts the capture. Returns false if it already started, or there's no memory.
bool slint_esp_trace_start()
{
    if (capture[0].load(relaxed)) {
        return false;
    }
    // PSRAM when there is some, so that the frame buffers and the renderer keep the internal RAM.
    uint32_t caps = MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT;
    size_t largest = heap_caps_get_largest_free_block(caps);
    if (largest == 0) {
        caps = MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT;
        largest = heap_caps_get_largest_free_block(caps);
    }
    const size_t after_reserve = largest > RESERVE_BYTES ? largest - RESERVE_BYTES : 0;
    const size_t bytes = std::min({ MAX_CAPTURE_BYTES, largest / 4, after_reserve });
    if (bytes < MIN_CAPTURE_BYTES) {
        return false;
    }
    void *memory = heap_caps_malloc(bytes, caps);
    if (!memory) {
        return false;
    }
    install_console_driver();
    const size_t words_per_core = bytes / 4 / CORES;
    for (size_t core = 0; core < CORES; ++core) {
        capture[core].store(static_cast<uint32_t *>(memory) + core * words_per_core, relaxed);
    }
    capacity.store(words_per_core, relaxed);
    // The records of the other cores start with their track.
    for (size_t core = 1; core < CORES; ++core) {
        uint32_t *words = capture[core].load(relaxed);
        words[0] = extended(CORE, 1);
        words[1] = CORE_TRACKS[core];
        len[core].store(2, relaxed);
    }
    reference_us.store(uint32_t(esp_timer_get_time()), relaxed);
    reference_cycles.store(cycles(), relaxed);
    recording.store(true, relaxed);

    ticks_per_us.store(esp_rom_get_cpu_ticks_per_us(), relaxed);
    const uint32_t clock[] = { extended(CLOCK, 1), ticks_per_us.load(relaxed) };
    push(clock, 2);
    return true;
}

// Stops the capture that `slint_esp_trace_start` began, when the subscriber can't be installed.
void slint_esp_trace_cancel()
{
    recording.store(false, relaxed);
}

// The two functions around the probe spans measure the cost of one span through the whole
// recording path, then drop the probe's records.
uint32_t slint_esp_trace_probe_begin()
{
    probe_len = len[core_id()].load(relaxed);
    return cycles();
}

void slint_esp_trace_probe_end(uint32_t start, uint32_t probes)
{
    const uint32_t span_cost = (cycles() - start) / probes;
    len[core_id()].store(probe_len, relaxed);
    const uint32_t words[] = { extended(SPAN_COST, 1), span_cost };
    push(words, 2);
    printf("slint trace: recording one span costs %lu cycles, %lu ns\n", (unsigned long)span_cost,
           (unsigned long)(span_cost * 1000 / ticks_per_us.load(relaxed)));

    const uint32_t per_us = ticks_per_us.load(relaxed);
    delay.store(DELAY_US * per_us, relaxed);
    end.store((DELAY_US + DURATION_US) * per_us, relaxed);
    installed.store(cycles(), relaxed);
}

void slint_esp_trace_name(uint32_t id, const char *name, size_t name_len)
{
    push_name(id, name, name_len);
}

void slint_esp_trace_enter(uint32_t id)
{
    record(id, ENTER);
}

void slint_esp_trace_exit(uint32_t id)
{
    record(id, EXIT);
}

void slint_esp_trace_event(uint32_t id)
{
    record(id, EVENT);
}

void slint_esp_trace_field(uint32_t name, uint64_t value)
{
    push_field(name, value);
}

}

#endif
