// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#ifdef SLINT_ESP_TRACING

#    include <atomic>
#    include <cstdint>

namespace slint_trace {

// A span or field name. Its address is the ID of the name in the capture, like the metadata
// address of a Rust span.
struct Site
{
    const char *name;
    std::atomic<bool> registered;
};

void enter(Site *site);
void exit(Site *site);
// Records a field of the next span entered.
void field(Site *site, int64_t value);
// Aligns the cycle count of the core this runs on with the first core's, so that both cores'
// records share one time line. Call it once on each other core that records.
void register_core();

struct Span
{
    Site *site;
    explicit Span(Site *site) : site(site) { enter(site); }
    ~Span() { exit(site); }
};

}

#    define SLINT_TRACE_CONCAT_(a, b) a##b
#    define SLINT_TRACE_CONCAT(a, b) SLINT_TRACE_CONCAT_(a, b)
// Records a span from here to the end of the scope, shown in Tracy next to the Rust spans.
#    define SLINT_TRACE_SPAN(name)                                                                 \
        static slint_trace::Site SLINT_TRACE_CONCAT(slint_trace_site_, __LINE__) { name, false };  \
        slint_trace::Span SLINT_TRACE_CONCAT(slint_trace_span_, __LINE__)(                         \
                &SLINT_TRACE_CONCAT(slint_trace_site_, __LINE__))
// Records a field of the next SLINT_TRACE_SPAN.
#    define SLINT_TRACE_FIELD(name, value)                                                         \
        do {                                                                                       \
            static slint_trace::Site site { name, false };                                         \
            slint_trace::field(&site, int64_t(value));                                             \
        } while (0)
#    define SLINT_TRACE_REGISTER_CORE() slint_trace::register_core()
#else
#    define SLINT_TRACE_SPAN(name)
#    define SLINT_TRACE_FIELD(name, value)
#    define SLINT_TRACE_REGISTER_CORE()
#endif
