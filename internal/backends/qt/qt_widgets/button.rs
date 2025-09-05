// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore qstyle unshade

use super::*;
use i_slint_core::graphics::euclid;

#[allow(nonstandard_style)]
#[allow(unused)]
mod standard_button {
    // Generated with
    // bindgen /usr/include/qt/QtWidgets/qstyle.h  --whitelist-type QStyle -- -I /usr/include/qt -xc++ | grep _StandardPixmap_ -A1
    pub const QStyle_StandardPixmap_SP_TitleBarMenuButton: QStyle_StandardPixmap = 0;
    pub const QStyle_StandardPixmap_SP_TitleBarMinButton: QStyle_StandardPixmap = 1;
    pub const QStyle_StandardPixmap_SP_TitleBarMaxButton: QStyle_StandardPixmap = 2;
    pub const QStyle_StandardPixmap_SP_TitleBarCloseButton: QStyle_StandardPixmap = 3;
    pub const QStyle_StandardPixmap_SP_TitleBarNormalButton: QStyle_StandardPixmap = 4;
    pub const QStyle_StandardPixmap_SP_TitleBarShadeButton: QStyle_StandardPixmap = 5;
    pub const QStyle_StandardPixmap_SP_TitleBarUnshadeButton: QStyle_StandardPixmap = 6;
    pub const QStyle_StandardPixmap_SP_TitleBarContextHelpButton: QStyle_StandardPixmap = 7;
    pub const QStyle_StandardPixmap_SP_DockWidgetCloseButton: QStyle_StandardPixmap = 8;
    pub const QStyle_StandardPixmap_SP_MessageBoxInformation: QStyle_StandardPixmap = 9;
    pub const QStyle_StandardPixmap_SP_MessageBoxWarning: QStyle_StandardPixmap = 10;
    pub const QStyle_StandardPixmap_SP_MessageBoxCritical: QStyle_StandardPixmap = 11;
    pub const QStyle_StandardPixmap_SP_MessageBoxQuestion: QStyle_StandardPixmap = 12;
    pub const QStyle_StandardPixmap_SP_DesktopIcon: QStyle_StandardPixmap = 13;
    pub const QStyle_StandardPixmap_SP_TrashIcon: QStyle_StandardPixmap = 14;
    pub const QStyle_StandardPixmap_SP_ComputerIcon: QStyle_StandardPixmap = 15;
    pub const QStyle_StandardPixmap_SP_DriveFDIcon: QStyle_StandardPixmap = 16;
    pub const QStyle_StandardPixmap_SP_DriveHDIcon: QStyle_StandardPixmap = 17;
    pub const QStyle_StandardPixmap_SP_DriveCDIcon: QStyle_StandardPixmap = 18;
    pub const QStyle_StandardPixmap_SP_DriveDVDIcon: QStyle_StandardPixmap = 19;
    pub const QStyle_StandardPixmap_SP_DriveNetIcon: QStyle_StandardPixmap = 20;
    pub const QStyle_StandardPixmap_SP_DirOpenIcon: QStyle_StandardPixmap = 21;
    pub const QStyle_StandardPixmap_SP_DirClosedIcon: QStyle_StandardPixmap = 22;
    pub const QStyle_StandardPixmap_SP_DirLinkIcon: QStyle_StandardPixmap = 23;
    pub const QStyle_StandardPixmap_SP_DirLinkOpenIcon: QStyle_StandardPixmap = 24;
    pub const QStyle_StandardPixmap_SP_FileIcon: QStyle_StandardPixmap = 25;
    pub const QStyle_StandardPixmap_SP_FileLinkIcon: QStyle_StandardPixmap = 26;
    pub const QStyle_StandardPixmap_SP_ToolBarHorizontalExtensionButton: QStyle_StandardPixmap = 27;
    pub const QStyle_StandardPixmap_SP_ToolBarVerticalExtensionButton: QStyle_StandardPixmap = 28;
    pub const QStyle_StandardPixmap_SP_FileDialogStart: QStyle_StandardPixmap = 29;
    pub const QStyle_StandardPixmap_SP_FileDialogEnd: QStyle_StandardPixmap = 30;
    pub const QStyle_StandardPixmap_SP_FileDialogToParent: QStyle_StandardPixmap = 31;
    pub const QStyle_StandardPixmap_SP_FileDialogNewFolder: QStyle_StandardPixmap = 32;
    pub const QStyle_StandardPixmap_SP_FileDialogDetailedView: QStyle_StandardPixmap = 33;
    pub const QStyle_StandardPixmap_SP_FileDialogInfoView: QStyle_StandardPixmap = 34;
    pub const QStyle_StandardPixmap_SP_FileDialogContentsView: QStyle_StandardPixmap = 35;
    pub const QStyle_StandardPixmap_SP_FileDialogListView: QStyle_StandardPixmap = 36;
    pub const QStyle_StandardPixmap_SP_FileDialogBack: QStyle_StandardPixmap = 37;
    pub const QStyle_StandardPixmap_SP_DirIcon: QStyle_StandardPixmap = 38;
    pub const QStyle_StandardPixmap_SP_DialogOkButton: QStyle_StandardPixmap = 39;
    pub const QStyle_StandardPixmap_SP_DialogCancelButton: QStyle_StandardPixmap = 40;
    pub const QStyle_StandardPixmap_SP_DialogHelpButton: QStyle_StandardPixmap = 41;
    pub const QStyle_StandardPixmap_SP_DialogOpenButton: QStyle_StandardPixmap = 42;
    pub const QStyle_StandardPixmap_SP_DialogSaveButton: QStyle_StandardPixmap = 43;
    pub const QStyle_StandardPixmap_SP_DialogCloseButton: QStyle_StandardPixmap = 44;
    pub const QStyle_StandardPixmap_SP_DialogApplyButton: QStyle_StandardPixmap = 45;
    pub const QStyle_StandardPixmap_SP_DialogResetButton: QStyle_StandardPixmap = 46;
    pub const QStyle_StandardPixmap_SP_DialogDiscardButton: QStyle_StandardPixmap = 47;
    pub const QStyle_StandardPixmap_SP_DialogYesButton: QStyle_StandardPixmap = 48;
    pub const QStyle_StandardPixmap_SP_DialogNoButton: QStyle_StandardPixmap = 49;
    pub const QStyle_StandardPixmap_SP_ArrowUp: QStyle_StandardPixmap = 50;
    pub const QStyle_StandardPixmap_SP_ArrowDown: QStyle_StandardPixmap = 51;
    pub const QStyle_StandardPixmap_SP_ArrowLeft: QStyle_StandardPixmap = 52;
    pub const QStyle_StandardPixmap_SP_ArrowRight: QStyle_StandardPixmap = 53;
    pub const QStyle_StandardPixmap_SP_ArrowBack: QStyle_StandardPixmap = 54;
    pub const QStyle_StandardPixmap_SP_ArrowForward: QStyle_StandardPixmap = 55;
    pub const QStyle_StandardPixmap_SP_DirHomeIcon: QStyle_StandardPixmap = 56;
    pub const QStyle_StandardPixmap_SP_CommandLink: QStyle_StandardPixmap = 57;
    pub const QStyle_StandardPixmap_SP_VistaShield: QStyle_StandardPixmap = 58;
    pub const QStyle_StandardPixmap_SP_BrowserReload: QStyle_StandardPixmap = 59;
    pub const QStyle_StandardPixmap_SP_BrowserStop: QStyle_StandardPixmap = 60;
    pub const QStyle_StandardPixmap_SP_MediaPlay: QStyle_StandardPixmap = 61;
    pub const QStyle_StandardPixmap_SP_MediaStop: QStyle_StandardPixmap = 62;
    pub const QStyle_StandardPixmap_SP_MediaPause: QStyle_StandardPixmap = 63;
    pub const QStyle_StandardPixmap_SP_MediaSkipForward: QStyle_StandardPixmap = 64;
    pub const QStyle_StandardPixmap_SP_MediaSkipBackward: QStyle_StandardPixmap = 65;
    pub const QStyle_StandardPixmap_SP_MediaSeekForward: QStyle_StandardPixmap = 66;
    pub const QStyle_StandardPixmap_SP_MediaSeekBackward: QStyle_StandardPixmap = 67;
    pub const QStyle_StandardPixmap_SP_MediaVolume: QStyle_StandardPixmap = 68;
    pub const QStyle_StandardPixmap_SP_MediaVolumeMuted: QStyle_StandardPixmap = 69;
    pub const QStyle_StandardPixmap_SP_LineEditClearButton: QStyle_StandardPixmap = 70;
    pub const QStyle_StandardPixmap_SP_DialogYesToAllButton: QStyle_StandardPixmap = 71;
    pub const QStyle_StandardPixmap_SP_DialogNoToAllButton: QStyle_StandardPixmap = 72;
    pub const QStyle_StandardPixmap_SP_DialogSaveAllButton: QStyle_StandardPixmap = 73;
    pub const QStyle_StandardPixmap_SP_DialogAbortButton: QStyle_StandardPixmap = 74;
    pub const QStyle_StandardPixmap_SP_DialogRetryButton: QStyle_StandardPixmap = 75;
    pub const QStyle_StandardPixmap_SP_DialogIgnoreButton: QStyle_StandardPixmap = 76;
    pub const QStyle_StandardPixmap_SP_RestoreDefaultsButton: QStyle_StandardPixmap = 77;
    pub const QStyle_StandardPixmap_SP_CustomBase: QStyle_StandardPixmap = 4026531840;
    pub type QStyle_StandardPixmap = ::std::os::raw::c_uint;
}

use i_slint_core::{
    input::{FocusEventResult, KeyEventType},
    items::StandardButtonKind,
    platform::PointerEventButton,
};
use standard_button::*;

type ActualStandardButtonKind = Option<StandardButtonKind>;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeButton {
    pub text: Property<SharedString>,
    pub icon: Property<i_slint_core::graphics::Image>,
    pub icon_size: Property<LogicalLength>,
    pub pressed: Property<bool>,
    pub has_hover: Property<bool>,
    pub checkable: Property<bool>,
    pub checked: Property<bool>,
    pub primary: Property<bool>,
    pub has_focus: Property<bool>,
    pub clicked: Callback<VoidArg>,
    pub enabled: Property<bool>,
    pub colorize_icon: Property<bool>,
    pub standard_button_kind: Property<StandardButtonKind>,
    pub is_standard_button: Property<bool>,
    widget_ptr: std::cell::Cell<SlintTypeErasedWidgetPtr>,
    animation_tracker: Property<i32>,
    pub cached_rendering_data: CachedRenderingData,
}

impl NativeButton {
    fn actual_standard_button_kind(self: Pin<&Self>) -> ActualStandardButtonKind {
        self.is_standard_button().then(|| self.standard_button_kind())
    }

    fn actual_text(
        self: Pin<&Self>,
        standard_button_kind: ActualStandardButtonKind,
    ) -> qttypes::QString {
        // We would need to use the private API to get the text from QPlatformTheme
        match standard_button_kind {
            Some(StandardButtonKind::Ok) => "OK".into(),
            Some(StandardButtonKind::Cancel) => "Cancel".into(),
            Some(StandardButtonKind::Apply) => "Apply".into(),
            Some(StandardButtonKind::Close) => "Close".into(),
            Some(StandardButtonKind::Reset) => "Reset".into(),
            Some(StandardButtonKind::Help) => "Help".into(),
            Some(StandardButtonKind::Yes) => "Yes".into(),
            Some(StandardButtonKind::No) => "No".into(),
            Some(StandardButtonKind::Abort) => "Abort".into(),
            Some(StandardButtonKind::Retry) => "Retry".into(),
            Some(StandardButtonKind::Ignore) => "Ignore".into(),
            None => self.text().as_str().into(),
        }
    }

    fn actual_icon(
        self: Pin<&Self>,
        standard_button_kind: ActualStandardButtonKind,
    ) -> qttypes::QPixmap {
        let style_icon = match standard_button_kind {
            Some(StandardButtonKind::Ok) => QStyle_StandardPixmap_SP_DialogOkButton,
            Some(StandardButtonKind::Cancel) => QStyle_StandardPixmap_SP_DialogCancelButton,
            Some(StandardButtonKind::Apply) => QStyle_StandardPixmap_SP_DialogApplyButton,
            Some(StandardButtonKind::Close) => QStyle_StandardPixmap_SP_DialogCloseButton,
            Some(StandardButtonKind::Reset) => QStyle_StandardPixmap_SP_DialogResetButton,
            Some(StandardButtonKind::Help) => QStyle_StandardPixmap_SP_DialogHelpButton,
            Some(StandardButtonKind::Yes) => QStyle_StandardPixmap_SP_DialogYesButton,
            Some(StandardButtonKind::No) => QStyle_StandardPixmap_SP_DialogNoButton,
            Some(StandardButtonKind::Abort) => QStyle_StandardPixmap_SP_DialogAbortButton,
            Some(StandardButtonKind::Retry) => QStyle_StandardPixmap_SP_DialogRetryButton,
            Some(StandardButtonKind::Ignore) => QStyle_StandardPixmap_SP_DialogIgnoreButton,
            None => {
                let icon_size = self.icon_size().get().round() as u32;
                let source_size = Some(euclid::Size2D::new(icon_size, icon_size));
                return crate::qt_window::image_to_pixmap((&self.icon()).into(), source_size)
                    .unwrap_or_default();
            }
        };
        let widget_ptr: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);
        cpp!(unsafe [style_icon as "QStyle::StandardPixmap", widget_ptr as "QWidget*"] -> qttypes::QPixmap as "QPixmap" {
            ensure_initialized();
            auto style = qApp->style();
            if (!style->styleHint(QStyle::SH_DialogButtonBox_ButtonsHaveIcons, nullptr, widget_ptr))
                return QPixmap();
            return style->standardPixmap(style_icon);
        })
    }

    fn activate(self: Pin<&Self>) {
        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(false);
        if self.checkable() {
            let checked = Self::FIELD_OFFSETS.checked.apply_pin(self);
            checked.set(!checked.get());
        }
        Self::FIELD_OFFSETS.clicked.apply_pin(self).call(&());
    }
}

impl Item for NativeButton {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        let animation_tracker_property_ptr = Self::FIELD_OFFSETS.animation_tracker.apply_pin(self);
        self.widget_ptr.set(cpp! { unsafe [animation_tracker_property_ptr as "void*"] -> SlintTypeErasedWidgetPtr as "std::unique_ptr<SlintTypeErasedWidget>" {
            return make_unique_animated_widget<QPushButton>(animation_tracker_property_ptr);
        }});
        let widget_ptr: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);
        let icon_size = unsafe {
            cpp!([widget_ptr as "QWidget*" ] -> i32 as "int"
            {
                ensure_initialized();
                return qApp->style()->pixelMetric(QStyle::PM_ButtonIconSize, 0, widget_ptr);
            })
        };
        Self::FIELD_OFFSETS
            .icon_size
            .apply_pin(self)
            .set(LogicalLength::new(icon_size as i_slint_core::Coord));
    }

    fn layout_info(
        self: Pin<&Self>,
        orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        let standard_button_kind = self.actual_standard_button_kind();
        let mut text: qttypes::QString = self.actual_text(standard_button_kind);
        let icon: qttypes::QPixmap = self.actual_icon(standard_button_kind);
        let icon_size = self.icon_size().get() as i32;
        let widget_ptr: NonNull<()> = SlintTypeErasedWidgetPtr::qwidget_ptr(&self.widget_ptr);
        let size = cpp!(unsafe [
            mut text as "QString",
            icon as "QPixmap",
            icon_size as "int",
            widget_ptr as "QWidget*"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            if (text.isEmpty())
                text = "**";
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            option.icon = icon;
            option.iconSize = QSize(icon_size, icon_size);
            if (!icon.isNull()) {
                option.rect.setHeight(qMax(option.rect.height(), icon_size));
                option.rect.setWidth(option.rect.width() + 4 + icon_size);
            }
            return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), widget_ptr);
        });
        let min = match orientation {
            Orientation::Horizontal => size.width as f32,
            Orientation::Vertical => size.height as f32,
        };
        LayoutInfo { min, preferred: min, ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(!matches!(event, MouseEvent::Exit));
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        if matches!(event, MouseEvent::Exit) {
            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false);
        }
        let enabled = self.enabled();
        if !enabled {
            return InputEventResult::EventIgnored;
        }

        let was_pressed = self.pressed();

        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event {
            MouseEvent::Pressed { button, .. } => *button == PointerEventButton::Left,
            MouseEvent::Exit | MouseEvent::Released { .. } => false,
            MouseEvent::Moved { .. } => {
                return if was_pressed {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                }
            }
            MouseEvent::Wheel { .. } => return InputEventResult::EventIgnored,
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => {
                return InputEventResult::EventIgnored
            }
        });
        if let MouseEvent::Released { position, .. } = event {
            let geo = self_rc.geometry();
            if LogicalRect::new(LogicalPoint::default(), geo.size).contains(*position)
                && was_pressed
            {
                self.activate();
            }
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        }
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _event: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        event: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        match event.event_type {
            KeyEventType::KeyPressed if event.text == " " || event.text == "\n" => {
                Self::FIELD_OFFSETS.pressed.apply_pin(self).set(true);
                KeyEventResult::EventAccepted
            }
            KeyEventType::KeyPressed => KeyEventResult::EventIgnored,
            KeyEventType::KeyReleased if event.text == " " || event.text == "\n" => {
                self.activate();
                KeyEventResult::EventAccepted
            }
            KeyEventType::KeyReleased => KeyEventResult::EventIgnored,
            KeyEventType::UpdateComposition | KeyEventType::CommitComposition => {
                KeyEventResult::EventIgnored
            }
        }
    }

    fn focus_event(
        self: Pin<&Self>,
        event: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        if self.enabled() {
            Self::FIELD_OFFSETS
                .has_focus
                .apply_pin(self)
                .set(matches!(event, FocusEvent::FocusIn(_)));
            FocusEventResult::FocusAccepted
        } else {
            FocusEventResult::FocusIgnored
        }
    }

    fn_render! { this dpr size painter widget initial_state =>
        let down: bool = this.pressed();
        let checked: bool = this.checked();
        let standard_button_kind = this.actual_standard_button_kind();
        let text: qttypes::QString = this.actual_text(standard_button_kind);
        let icon: qttypes::QPixmap = this.actual_icon(standard_button_kind);
        let enabled = this.enabled();
        let has_focus = this.has_focus();
        let has_hover = this.has_hover();
        let primary = this.primary();
        let icon_size = this.icon_size().get().round() as i32;
        let colorize_icon = this.colorize_icon();

        cpp!(unsafe [
            painter as "QPainterPtr*",
            widget as "QWidget*",
            text as "QString",
            icon as "QPixmap",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            checked as "bool",
            has_focus as "bool",
            has_hover as "bool",
            primary as "bool",
            icon_size as "int",
            colorize_icon as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            class ColorizedIconEngine : public QIconEngine
            {
            public:
                ColorizedIconEngine(const QIcon &icon, const QColor &color) : m_icon(icon), m_color(color) { }

                QPixmap pixmap(const QSize &size, QIcon::Mode mode, QIcon::State state) override
                {
                    QPixmap iconPixmap = m_icon.pixmap(size, mode, state);
                    if (!iconPixmap.isNull()) {
                        QPainter colorizePainter(&iconPixmap);
                        colorizePainter.setCompositionMode(QPainter::CompositionMode_SourceIn);
                        colorizePainter.fillRect(iconPixmap.rect(), m_color);
                    }
                    return iconPixmap;
                }

                void paint(QPainter *painter, const QRect &rect, QIcon::Mode mode, QIcon::State state) override
                {
                    painter->drawPixmap(rect, this->pixmap(rect.size(), mode, state));
                }

                QIconEngine *clone() const override { return new ColorizedIconEngine(m_icon, m_color); }

            private:
                QIcon m_icon;
                QColor m_color;
            };

            QStyleOptionButton option;
            option.styleObject = widget;
            option.state |= QStyle::State(initial_state);
            option.text = std::move(text);

            QColor iconColor = qApp->palette().color(QPalette::ButtonText).rgba();

            if (down) {
                option.state |= QStyle::State_Sunken;
            } else {
                option.state |= QStyle::State_Raised;
            }
            if (checked) {
                option.state |= QStyle::State_On;
            }
            if (enabled) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
                iconColor = qApp->palette().color(QPalette::Disabled, QPalette::ButtonText).rgba();
            }
            if (has_focus) {
                option.state |= QStyle::State_HasFocus | QStyle::State_KeyboardFocusChange | QStyle::State_Item;
            }
            if (has_hover) {
                option.state |= QStyle::State_MouseOver;
            }
            if (primary) {
                option.features |= QStyleOptionButton::DefaultButton;
            }
            if (colorize_icon) {
                option.icon = QIcon(new ColorizedIconEngine(icon, iconColor));
            } else {
                option.icon = icon;
            }
            option.iconSize = QSize(icon_size, icon_size);
            option.rect = QRect(QPoint(), size / dpr);

            qApp->style()->drawControl(QStyle::CE_PushButton, &option, painter->get(), widget);
        });
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: LogicalRect,
    ) -> LogicalRect {
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for NativeButton {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_NativeButtonVTable() -> NativeButtonVTable for NativeButton
}
