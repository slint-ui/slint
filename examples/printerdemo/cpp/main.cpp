// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "printerdemo.h"

#include <ctime>

struct InkLevelModel : slint::Model<InkLevel>
{
    size_t row_count() const override { return m_data.size(); }
    std::optional<InkLevel> row_data(size_t i) const override
    {
        if (i < row_count())
            return { m_data[i] };
        return {};
    }

    std::vector<InkLevel> m_data = { { slint::Color::from_rgb_uint8(255, 255, 0), 0.9 },
                                     { slint::Color::from_rgb_uint8(0, 255, 255), 0.5 },
                                     { slint::Color::from_rgb_uint8(255, 0, 255), 0.8 },
                                     { slint::Color::from_rgb_uint8(0, 0, 0), 0.1 } };
};

int main()
{
    auto printer_demo = MainWindow::create();
    printer_demo->set_ink_levels(std::make_shared<InkLevelModel>());
    printer_demo->on_quit([] { std::exit(0); });

    auto printer_queue = std::make_shared<slint::VectorModel<PrinterQueueItem>>();
    auto default_queue = printer_demo->global<PrinterQueue>().get_printer_queue();
    for (int i = 0; i < default_queue->row_count(); ++i) {
        printer_queue->push_back(*default_queue->row_data(i));
    }
    printer_demo->global<PrinterQueue>().set_printer_queue(printer_queue);

    printer_demo->global<PrinterQueue>().on_start_job([=](slint::SharedString name) {
        std::time_t now = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
        char time_buf[100] = { 0 };
        std::strftime(time_buf, sizeof(time_buf), "%H:%M:%S %d/%m/%Y", std::localtime(&now));
        PrinterQueueItem item;
        item.status = "WAITING...";
        item.progress = 0;
        item.title = std::move(name);
        item.owner = "joe@example.com";
        item.pages = 1;
        item.size = "100kB";
        item.submission_date = time_buf;
        printer_queue->push_back(item);
    });

    printer_demo->global<PrinterQueue>().on_cancel_job(
            [=](int index) { printer_queue->erase(int(index)); });

    slint::Timer printer_queue_progress_timer(std::chrono::seconds(1), [=]() {
        if (printer_queue->row_count() > 0) {
            auto top_item = *printer_queue->row_data(0);
            top_item.progress += 1;
            if (top_item.progress > 100) {
                printer_queue->erase(0);
            } else {
                top_item.status = "PRINTING";
                printer_queue->set_row_data(0, top_item);
            }
        }
    });

    printer_demo->run();
}
