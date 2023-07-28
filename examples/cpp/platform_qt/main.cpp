// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "appwindow.h"

#include <slint-platform.h>

#include <QtGui/QtGui>
#include <QtGui/qpa/qplatformnativeinterface.h>
#include <QtWidgets/QApplication>

slint::PointerEventButton convert_button(Qt::MouseButtons b)
{
    switch (b) {
    case Qt::LeftButton:
        return slint::PointerEventButton::Left;
    case Qt::RightButton:
        return slint::PointerEventButton::Right;
    case Qt::MiddleButton:
        return slint::PointerEventButton::Middle;
    default:
        return slint::PointerEventButton::Other;
    }
}

static slint::platform::NativeWindowHandle window_handle_for_qt_window(QWindow *window)
{
    // Ensure that the native window surface exists
    window->create();
#ifdef __APPLE__
    QPlatformNativeInterface *native = qApp->platformNativeInterface();
    NSView *nsview = reinterpret_cast<NSView *>(
            native->nativeResourceForWindow(QByteArray("nsview"), window));
    NSWindow *nswindow = reinterpret_cast<NSWindow *>(
            native->nativeResourceForWindow(QByteArray("nswindow"), window));
    return slint::platform::NativeWindowHandle::from_appkit(nsview, nswindow);
#elif defined Q_OS_WIN
    auto wid = Qt::HANDLE(window->winId());
    return slint::platform::NativeWindowHandle::from_win32(wid, GetModuleHandle(nullptr));
#else
    // Try Wayland first, then XLib, then Xcb
    auto wid = window->winId();
    auto visual_id = 0; // FIXME
    QPlatformNativeInterface *native = qApp->platformNativeInterface();
    auto screen = quintptr(native->nativeResourceForWindow(QByteArray("x11screen"), window));
    if (auto *wayland_display = reinterpret_cast<wl_display *>(
                native->nativeResourceForIntegration(QByteArray("wl_display")))) {
        auto *wayland_surface = reinterpret_cast<wl_surface *>(
                native->nativeResourceForWindow(QByteArray("surface"), window));
        return slint::platform::NativeWindowHandle::from_wayland(wayland_surface, wayland_display);
    } else if (auto *x11_display = native->nativeResourceForWindow(QByteArray("display"), window)) {
        return slint::platform::NativeWindowHandle::from_x11_xlib(wid, wid, x11_display, screen);
    } else if (auto *xcb_connection = reinterpret_cast<xcb_connection_t *>(
                       native->nativeResourceForWindow(QByteArray("connection"), window))) {
        return slint::platform::NativeWindowHandle::from_x11_xcb(wid, wid, xcb_connection, screen);
    } else {
        throw "Unsupported windowing system (tried wayland, xlib, and xcb)";
    }
#endif
}

static slint::SharedString key_event_text(QKeyEvent *e)
{
    // TODO: handle special keys
    return e->text().toUtf8().data();
}

class MyWindow : public QWindow, public slint::platform::WindowAdapter
{
    std::optional<slint::platform::SkiaRenderer> m_renderer;

public:
    MyWindow(QWindow *parentWindow = nullptr) : QWindow(parentWindow)
    {
        resize(640, 480);
        m_renderer.emplace(window_handle_for_qt_window(this), physical_size());
    }

    slint::platform::AbstractRenderer &renderer() override { return m_renderer.value(); }

    void paintEvent(QPaintEvent *ev) override
    {
        slint::platform::update_timers_and_animations();

        m_renderer->render();

        if (window().has_active_animations()) {
            requestUpdate();
        }
    }

    bool event(QEvent *e) override
    {
        if (e->type() == QEvent::UpdateRequest) {
            paintEvent(static_cast<QPaintEvent *>(e));
            return true;
        } else if (e->type() == QEvent::KeyPress) {
            window().dispatch_key_press_event(key_event_text(static_cast<QKeyEvent *>(e)));
            return true;
        } else if (e->type() == QEvent::KeyRelease) {
            window().dispatch_key_release_event(key_event_text(static_cast<QKeyEvent *>(e)));
            return true;
        } else {
            return QWindow::event(e);
        }
    }

    void set_visible(bool visible) override
    {
        if (visible) {
            window().dispatch_scale_factor_change_event(devicePixelRatio());
        }
        this->QWindow::setVisible(visible);
    }

    slint::PhysicalSize physical_size() const override
    {
        auto windowSize = slint::LogicalSize({ float(width()), float(height()) });
        float scale_factor = devicePixelRatio();
        return slint::PhysicalSize({ uint32_t(windowSize.width * scale_factor),
                                     uint32_t(windowSize.height * scale_factor) });
    }

    void request_redraw() override { requestUpdate(); }

    void resizeEvent(QResizeEvent *ev) override
    {
        auto logicalSize = ev->size();
        window().dispatch_resize_event(
                slint::LogicalSize({ float(logicalSize.width()), float(logicalSize.height()) }));
    }

    void mousePressEvent(QMouseEvent *event) override
    {
        slint::platform::update_timers_and_animations();
        window().dispatch_pointer_press_event(
                slint::LogicalPosition({ float(event->pos().x()), float(event->pos().y()) }),
                convert_button(event->button()));
    }
    void mouseReleaseEvent(QMouseEvent *event) override
    {
        slint::platform::update_timers_and_animations();
        window().dispatch_pointer_release_event(
                slint::LogicalPosition({ float(event->pos().x()), float(event->pos().y()) }),
                convert_button(event->button()));
    }
    void mouseMoveEvent(QMouseEvent *event) override
    {
        slint::platform::update_timers_and_animations();
        window().dispatch_pointer_move_event(
                slint::LogicalPosition({ float(event->pos().x()), float(event->pos().y()) }));
    }
};

struct MyPlatform : public slint::platform::Platform
{

    std::unique_ptr<QWindow> parentWindow;

    std::unique_ptr<slint::platform::WindowAdapter> create_window_adapter() override
    {
        return std::make_unique<MyWindow>(parentWindow.get());
    }
};

int main(int argc, char **argv)
{
    QApplication app(argc, argv);

    static MyPlatform *plarform = [] {
        auto platform = std::make_unique<MyPlatform>();
        auto p2 = platform.get();
        slint::platform::set_platform(std::move(platform));
        return p2;
    }();

    slint::platform::update_timers_and_animations();

    auto my_ui = App::create();
    // mu_ui->set_property(....);
    my_ui->show();

    return app.exec();
}
