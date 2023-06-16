// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

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

static slint_platform::NativeWindowHandle window_handle_for_qt_window(QWindow *window)
{
    // Ensure that the native window surface exists
    window->create();
#ifdef __APPLE__
    QPlatformNativeInterface *native = qApp->platformNativeInterface();
    void *nsview = native->nativeResourceForWindow(QByteArray("nsview"), window);
    void *nswindow = native->nativeResourceForWindow(QByteArray("nswindow"), window);
    return slint_platform::NativeWindowHandle::from_appkit(nsview, nswindow);
#elif defined Q_OS_WIN
    auto wid = Qt::HANDLE(window->winId());
    return slint_platform::NativeWindowHandle::from_win32(wid, GetModuleHandle(nullptr));
#else
    auto wid = window->winId();
    auto visual_id = 0; // FIXME
    QPlatformNativeInterface *native = qApp->platformNativeInterface();
    auto *connection = reinterpret_cast<xcb_connection_t *>(
            native->nativeResourceForWindow(QByteArray("connection"), window));
    auto screen = quintptr(native->nativeResourceForWindow(QByteArray("screen"), window));

    return slint_platform::NativeWindowHandle::from_x11(wid, wid, connection, screen);
#endif
}

class MyWindow : public QWindow, public slint_platform::WindowAdapter
{
    std::optional<slint_platform::SkiaRenderer> m_renderer;

public:
    MyWindow(QWindow *parentWindow = nullptr) : QWindow(parentWindow)
    {
        m_renderer.emplace(window_handle_for_qt_window(this), physical_size());
    }

    slint_platform::AbstractRenderer &renderer() override { return m_renderer.value(); }

    /*void keyEvent(QKeyEvent *event) override
    {
        renderer()->dispatch_key_event(slint::cbingen_private::UglyEnum {... })
    }*/

    void paintEvent(QPaintEvent *ev) override
    {
        slint_platform::update_timers_and_animations();

        m_renderer->render(window());

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
        const_cast<WindowAdapter *>(static_cast<const WindowAdapter *>(this))
                ->dispatch_scale_factor_change_event(devicePixelRatio());
        auto window = const_cast<QWindow *>(static_cast<const QWindow *>(this));
        window->QWindow::show();
        m_renderer->show();
    }
    void hide() const override
    {
        m_renderer->hide();
        const_cast<MyWindow *>(this)->QWindow::hide();
    }
    slint::PhysicalSize physical_size() const override
    {
        auto windowSize = slint::LogicalSize({ float(width()), float(height()) });
        float scale_factor = devicePixelRatio();
        return slint::PhysicalSize({ uint32_t(windowSize.width * scale_factor),
                                     uint32_t(windowSize.height * scale_factor) });
    }

    void request_redraw() const override { const_cast<MyWindow *>(this)->requestUpdate(); }

    void resizeEvent(QResizeEvent *ev) override
    {
        auto logicalSize = ev->size();
        float scale_factor = devicePixelRatio();
        m_renderer->resize(slint::PhysicalSize({ uint32_t(logicalSize.width() * scale_factor),
                                                 uint32_t(logicalSize.height() * scale_factor) }));
        WindowAdapter::dispatch_resize_event(
                slint::LogicalSize({ float(logicalSize.width()), float(logicalSize.height()) }));
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

    std::unique_ptr<slint_platform::WindowAdapter> create_window_adapter() const override
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
