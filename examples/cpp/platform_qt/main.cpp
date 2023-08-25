// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "appwindow.h"

#include <slint-platform.h>

#include <QtGui/QtGui>
#include <QtGui/qpa/qplatformnativeinterface.h>
#include <QtWidgets/QApplication>

static void update_timer()
{
    static QTimer timer;
    static auto init = [] {
        timer.callOnTimeout([] {
            slint::platform::update_timers_and_animations();
            update_timer();
        });
        return true;
    }();
    if (auto timeout = slint::platform::duration_until_next_timer_update()) {
        timer.start(*timeout);
    } else {
        timer.stop();
    }
}

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
    switch (e->key()) {
    case Qt::Key::Key_Backspace:
        return slint::platform::key_codes::Backspace;
    case Qt::Key::Key_Tab:
        return slint::platform::key_codes::Tab;
    case Qt::Key::Key_Enter:
    case Qt::Key::Key_Return:
        return slint::platform::key_codes::Return;
    case Qt::Key::Key_Escape:
        return slint::platform::key_codes::Escape;
    case Qt::Key::Key_Backtab:
        return slint::platform::key_codes::Backtab;
    case Qt::Key::Key_Delete:
        return slint::platform::key_codes::Delete;
    case Qt::Key::Key_Shift:
        return slint::platform::key_codes::Shift;
    case Qt::Key::Key_Control:
        return slint::platform::key_codes::Control;
    case Qt::Key::Key_Alt:
        return slint::platform::key_codes::Alt;
    case Qt::Key::Key_AltGr:
        return slint::platform::key_codes::AltGr;
    case Qt::Key::Key_CapsLock:
        return slint::platform::key_codes::CapsLock;
    case Qt::Key::Key_Meta:
        return slint::platform::key_codes::Meta;
    case Qt::Key::Key_Up:
        return slint::platform::key_codes::UpArrow;
    case Qt::Key::Key_Down:
        return slint::platform::key_codes::DownArrow;
    case Qt::Key::Key_Left:
        return slint::platform::key_codes::LeftArrow;
    case Qt::Key::Key_Right:
        return slint::platform::key_codes::RightArrow;
    case Qt::Key::Key_F1:
        return slint::platform::key_codes::F1;
    case Qt::Key::Key_F2:
        return slint::platform::key_codes::F2;
    case Qt::Key::Key_F3:
        return slint::platform::key_codes::F3;
    case Qt::Key::Key_F4:
        return slint::platform::key_codes::F4;
    case Qt::Key::Key_F5:
        return slint::platform::key_codes::F5;
    case Qt::Key::Key_F6:
        return slint::platform::key_codes::F6;
    case Qt::Key::Key_F7:
        return slint::platform::key_codes::F7;
    case Qt::Key::Key_F8:
        return slint::platform::key_codes::F8;
    case Qt::Key::Key_F9:
        return slint::platform::key_codes::F9;
    case Qt::Key::Key_F10:
        return slint::platform::key_codes::F10;
    case Qt::Key::Key_F11:
        return slint::platform::key_codes::F11;
    case Qt::Key::Key_F12:
        return slint::platform::key_codes::F12;
    case Qt::Key::Key_F13:
        return slint::platform::key_codes::F13;
    case Qt::Key::Key_F14:
        return slint::platform::key_codes::F14;
    case Qt::Key::Key_F15:
        return slint::platform::key_codes::F15;
    case Qt::Key::Key_F16:
        return slint::platform::key_codes::F16;
    case Qt::Key::Key_F17:
        return slint::platform::key_codes::F17;
    case Qt::Key::Key_F18:
        return slint::platform::key_codes::F18;
    case Qt::Key::Key_F19:
        return slint::platform::key_codes::F19;
    case Qt::Key::Key_F20:
        return slint::platform::key_codes::F20;
    case Qt::Key::Key_F21:
        return slint::platform::key_codes::F21;
    case Qt::Key::Key_F22:
        return slint::platform::key_codes::F22;
    case Qt::Key::Key_F23:
        return slint::platform::key_codes::F23;
    case Qt::Key::Key_F24:
        return slint::platform::key_codes::F24;
    case Qt::Key::Key_Insert:
        return slint::platform::key_codes::Insert;
    case Qt::Key::Key_Home:
        return slint::platform::key_codes::Home;
    case Qt::Key::Key_End:
        return slint::platform::key_codes::End;
    case Qt::Key::Key_PageUp:
        return slint::platform::key_codes::PageUp;
    case Qt::Key::Key_PageDown:
        return slint::platform::key_codes::PageDown;
    case Qt::Key::Key_ScrollLock:
        return slint::platform::key_codes::ScrollLock;
    case Qt::Key::Key_Pause:
        return slint::platform::key_codes::Pause;
    case Qt::Key::Key_SysReq:
        return slint::platform::key_codes::SysReq;
    case Qt::Key::Key_Stop:
        return slint::platform::key_codes::Stop;
    case Qt::Key::Key_Menu:
        return slint::platform::key_codes::Menu;
    default:
        if (e->modifiers() & Qt::ControlModifier) {
            // e->text() is not the key when Ctrl is pressed
            return QKeySequence(e->key()).toString().toLower().toUtf8().data();
        }
        return e->text().toUtf8().data();
    }
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
        update_timer();
    }

    void closeEvent(QCloseEvent *event) override
    {
        window().dispatch_close_requested_event();
        event->ignore();
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
        } else if (e->type() == QEvent::WindowActivate) {
            window().dispatch_window_active_changed_event(true);
            return true;
        } else if (e->type() == QEvent::WindowDeactivate) {
            window().dispatch_window_active_changed_event(false);
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

    void update_window_properties(const slint::platform::WindowProperties &props) override
    {
        QWindow::setTitle(QString::fromUtf8(props.title().data()));
        auto c = props.layout_constraints();
        QWindow::setMaximumSize(c.max ? QSize(c.max->width, c.max->height)
                                      : QSize(1 << 15, 1 << 15));
        QWindow::setMinimumSize(c.min ? QSize(c.min->width, c.min->height) : QSize());
    }

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
        update_timer();
    }
    void mouseReleaseEvent(QMouseEvent *event) override
    {
        slint::platform::update_timers_and_animations();
        window().dispatch_pointer_release_event(
                slint::LogicalPosition({ float(event->pos().x()), float(event->pos().y()) }),
                convert_button(event->button()));
        update_timer();
    }
    void mouseMoveEvent(QMouseEvent *event) override
    {
        slint::platform::update_timers_and_animations();
        window().dispatch_pointer_move_event(
                slint::LogicalPosition({ float(event->pos().x()), float(event->pos().y()) }));
        update_timer();
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
