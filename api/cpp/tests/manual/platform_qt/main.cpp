// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include "appwindow.h"

#include "slint_platform.h"

#include <QtGui/QtGui>
#include <QtGui/qpa/qplatformnativeinterface.h>
#include <QtWidgets/QApplication>

namespace slint_platform = slint::experimental::platform;

slint::cbindgen_private::PointerEventButton convert_button(Qt::MouseButtons b)
{
    switch (b) {
    case Qt::LeftButton:
        return slint::cbindgen_private::PointerEventButton::Left;
    case Qt::RightButton:
        return slint::cbindgen_private::PointerEventButton::Right;
    case Qt::MiddleButton:
        return slint::cbindgen_private::PointerEventButton::Middle;
    default:
        return slint::cbindgen_private::PointerEventButton::Other;
    }
}

class MyWindow : public QWindow, public slint_platform::WindowAdapter<slint_platform::SkiaRenderer>
{

public:
    MyWindow(QWindow *parentWindow = nullptr) : QWindow(parentWindow) { }

    /*void keyEvent(QKeyEvent *event) override
    {
        renderer()->dispatch_key_event(slint::cbingen_private::UglyEnum {... })
    }*/

    void paintEvent(QPaintEvent *ev) override
    {
        slint_platform::update_timers_and_animations();

        auto windowSize = slint::PhysicalSize({ uint32_t(width()), uint32_t(height()) });
        renderer().render(windowSize);

        if (has_active_animations()) {
            requestUpdate();
        }
    }

    bool event(QEvent *e) override
    {
        if (e->type() == QEvent::UpdateRequest) {
            paintEvent(static_cast<QPaintEvent *>(e));
            return true;
        } else {
            return QWindow::event(e);
        }
    }

    void show() const override
    {
        auto window = const_cast<QWindow *>(static_cast<const QWindow *>(this));
        window->QWindow::show();
        auto windowSize = slint::PhysicalSize({ uint32_t(width()), uint32_t(height()) });
#ifdef __APPLE__
        QPlatformNativeInterface *native = qApp->platformNativeInterface();
        void *nsview = native->nativeResourceForWindow(QByteArray("nsview"), window);
        void *nswindow = native->nativeResourceForWindow(QByteArray("nswindow"), window);
        renderer().show(nsview, nswindow, windowSize);
#elif defined Q_OS_WIN
        auto wid = Qt::HANDLE(winId());
        renderer().show(
                wid, GetModuleHandle(nullptr),
                slint_platform::WindowAdapter<slint_platform::SkiaRenderer>::window().size());
#else
        auto wid = winId();
        auto visual_id = 0; // FIXME
        QPlatformNativeInterface *native = qApp->platformNativeInterface();
        auto *connection = reinterpret_cast<xcb_connection_t *>(
                native->nativeResourceForWindow(QByteArray("connection"), window));
        auto screen = quintptr(native->nativeResourceForWindow(QByteArray("screen"), window));

        renderer().show(
                wid, wid, connection, screen,
                slint_platform::WindowAdapter<slint_platform::SkiaRenderer>::window().size());
#endif
    }
    void hide() const override
    {
        renderer().hide();
        const_cast<MyWindow *>(this)->QWindow::hide();
    }

    void request_redraw() const override
    {
        const_cast<MyWindow *>(this)->requestUpdate();
    }

    void resizeEvent(QResizeEvent *ev) override
    {
        auto windowSize = slint::PhysicalSize(
                { uint32_t(ev->size().width()), uint32_t(ev->size().height()) });
        renderer().resize(windowSize);
        slint_platform::WindowAdapter<slint_platform::SkiaRenderer>::window().set_size(windowSize);
    }

    void mousePressEvent(QMouseEvent *event) override
    {
        slint_platform::update_timers_and_animations();
        dispatch_pointer_event(slint::cbindgen_private::MouseEvent {
                .tag = slint::cbindgen_private::MouseEvent::Tag::Pressed,
                .pressed = slint::cbindgen_private::MouseEvent::Pressed_Body {
                        .position = { float(event->pos().x()), float(event->pos().y()) },
                        .button = convert_button(event->button()) } });
    }
    void mouseReleaseEvent(QMouseEvent *event) override
    {
        slint_platform::update_timers_and_animations();
        dispatch_pointer_event(slint::cbindgen_private::MouseEvent {
                .tag = slint::cbindgen_private::MouseEvent::Tag::Released,
                .released = slint::cbindgen_private::MouseEvent::Released_Body {
                        .position = { float(event->pos().x()), float(event->pos().y()) },
                        .button = convert_button(event->button()) } });
    }
    void mouseMoveEvent(QMouseEvent *event) override
    {
        slint_platform::update_timers_and_animations();
        dispatch_pointer_event(slint::cbindgen_private::MouseEvent {
                .tag = slint::cbindgen_private::MouseEvent::Tag::Moved,
                .moved = slint::cbindgen_private::MouseEvent::Moved_Body {
                        .position = { float(event->pos().x()), float(event->pos().y()) },
                } });
    }
};

struct MyPlatform : public slint_platform::Platform
{

    std::unique_ptr<QWindow> parentWindow;

    std::unique_ptr<slint_platform::AbstractWindowAdapter> create_window_adapter() const override
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
        MyPlatform::register_platform(std::move(platform));
        return p2;
    }();

    slint_platform::update_timers_and_animations();

    auto my_ui = App::create();
    // mu_ui->set_property(....);
    my_ui->show();

    return app.exec();
}
