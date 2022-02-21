// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use super::*;

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

use i_slint_core::{input::FocusEventResult, items::StandardButtonKind};
use standard_button::*;

type ActualStandardButtonKind = Option<StandardButtonKind>;

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct NativeButton {
    pub x: Property<f32>,
    pub y: Property<f32>,
    pub width: Property<f32>,
    pub height: Property<f32>,
    pub text: Property<SharedString>,
    pub icon: Property<i_slint_core::graphics::Image>,
    pub enabled: Property<bool>,
    pub pressed: Property<bool>,
    pub clicked: Callback<VoidArg>,
    pub standard_button_kind: Property<StandardButtonKind>,
    pub is_standard_button: Property<bool>,
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
            Some(StandardButtonKind::ok) => "OK".into(),
            Some(StandardButtonKind::cancel) => "Cancel".into(),
            Some(StandardButtonKind::apply) => "Apply".into(),
            Some(StandardButtonKind::close) => "Close".into(),
            Some(StandardButtonKind::reset) => "Reset".into(),
            Some(StandardButtonKind::help) => "Help".into(),
            Some(StandardButtonKind::yes) => "Yes".into(),
            Some(StandardButtonKind::no) => "No".into(),
            Some(StandardButtonKind::abort) => "Abort".into(),
            Some(StandardButtonKind::retry) => "Retry".into(),
            Some(StandardButtonKind::ignore) => "Ignore".into(),
            None => self.text().as_str().into(),
        }
    }

    fn actual_icon(
        self: Pin<&Self>,
        standard_button_kind: ActualStandardButtonKind,
    ) -> qttypes::QPixmap {
        let style_icon = match standard_button_kind {
            Some(StandardButtonKind::ok) => QStyle_StandardPixmap_SP_DialogOkButton,
            Some(StandardButtonKind::cancel) => QStyle_StandardPixmap_SP_DialogCancelButton,
            Some(StandardButtonKind::apply) => QStyle_StandardPixmap_SP_DialogApplyButton,
            Some(StandardButtonKind::close) => QStyle_StandardPixmap_SP_DialogCloseButton,
            Some(StandardButtonKind::reset) => QStyle_StandardPixmap_SP_DialogResetButton,
            Some(StandardButtonKind::help) => QStyle_StandardPixmap_SP_DialogHelpButton,
            Some(StandardButtonKind::yes) => QStyle_StandardPixmap_SP_DialogYesButton,
            Some(StandardButtonKind::no) => QStyle_StandardPixmap_SP_DialogNoButton,
            Some(StandardButtonKind::abort) => QStyle_StandardPixmap_SP_DialogAbortButton,
            Some(StandardButtonKind::retry) => QStyle_StandardPixmap_SP_DialogRetryButton,
            Some(StandardButtonKind::ignore) => QStyle_StandardPixmap_SP_DialogIgnoreButton,
            None => {
                return crate::qt_window::load_image_from_resource(
                    (&self.icon()).into(),
                    None,
                    Default::default(),
                )
                .unwrap_or_default();
            }
        };
        cpp!(unsafe [style_icon as "QStyle::StandardPixmap"] -> qttypes::QPixmap as "QPixmap" {
            ensure_initialized();
            auto style = qApp->style();
            if (!style->styleHint(QStyle::SH_DialogButtonBox_ButtonsHaveIcons, nullptr, nullptr))
                return QPixmap();
            return style->standardPixmap(style_icon);
        })
    }
}

impl Item for NativeButton {
    fn init(self: Pin<&Self>, _window: &WindowRc) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(self: Pin<&Self>, orientation: Orientation, _window: &WindowRc) -> LayoutInfo {
        let standard_button_kind = self.actual_standard_button_kind();
        let mut text: qttypes::QString = self.actual_text(standard_button_kind);
        let icon: qttypes::QPixmap = self.actual_icon(standard_button_kind);
        let size = cpp!(unsafe [
            mut text as "QString",
            icon as "QPixmap"
        ] -> qttypes::QSize as "QSize" {
            ensure_initialized();
            QStyleOptionButton option;
            if (text.isEmpty())
                text = "**";
            option.rect = option.fontMetrics.boundingRect(text);
            option.text = std::move(text);
            option.icon = icon;
            auto iconSize = qApp->style()->pixelMetric(QStyle::PM_ButtonIconSize, 0, nullptr);
            option.iconSize = QSize(iconSize, iconSize);
            if (!icon.isNull()) {
                option.rect.setHeight(qMax(option.rect.height(), iconSize));
                option.rect.setWidth(option.rect.width() + 4 + iconSize);
            }
            return qApp->style()->sizeFromContents(QStyle::CT_PushButton, &option, option.rect.size(), nullptr);
        });
        LayoutInfo {
            min: match orientation {
                Orientation::Horizontal => size.width as f32,
                Orientation::Vertical => size.height as f32,
            },
            ..LayoutInfo::default()
        }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window: &WindowRc,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window: &WindowRc,
        _self_rc: &i_slint_core::items::ItemRc,
    ) -> InputEventResult {
        let enabled = self.enabled();
        if !enabled {
            return InputEventResult::EventIgnored;
        }

        Self::FIELD_OFFSETS.pressed.apply_pin(self).set(match event {
            MouseEvent::MousePressed { .. } => true,
            MouseEvent::MouseExit | MouseEvent::MouseReleased { .. } => false,
            MouseEvent::MouseMoved { .. } => {
                return if self.pressed() {
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::MouseWheel { .. } => return InputEventResult::EventIgnored,
        });
        if let MouseEvent::MouseReleased { pos, .. } = event {
            if euclid::rect(0., 0., self.width(), self.height()).contains(pos) {
                Self::FIELD_OFFSETS.clicked.apply_pin(self).call(&());
            }
            InputEventResult::EventAccepted
        } else {
            InputEventResult::GrabMouse
        }
    }

    fn key_event(self: Pin<&Self>, _: &KeyEvent, _window: &WindowRc) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(self: Pin<&Self>, _: &FocusEvent, _window: &WindowRc) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn_render! { this dpr size painter widget initial_state =>
        let down: bool = this.pressed();
        let standard_button_kind = this.actual_standard_button_kind();
        let text: qttypes::QString = this.actual_text(standard_button_kind);
        let icon: qttypes::QPixmap = this.actual_icon(standard_button_kind);
        let enabled = this.enabled();

        cpp!(unsafe [
            painter as "QPainter*",
            widget as "QWidget*",
            text as "QString",
            icon as "QPixmap",
            enabled as "bool",
            size as "QSize",
            down as "bool",
            dpr as "float",
            initial_state as "int"
        ] {
            QStyleOptionButton option;
            option.state |= QStyle::State(initial_state);
            option.text = std::move(text);
            option.icon = icon;
            auto iconSize = qApp->style()->pixelMetric(QStyle::PM_ButtonIconSize, 0, nullptr);
            option.iconSize = QSize(iconSize, iconSize);
            option.rect = QRect(QPoint(), size / dpr);
            if (down)
                option.state |= QStyle::State_Sunken;
            else
                option.state |= QStyle::State_Raised;
            if (enabled) {
                option.state |= QStyle::State_Enabled;
            } else {
                option.palette.setCurrentColorGroup(QPalette::Disabled);
            }
            qApp->style()->drawControl(QStyle::CE_PushButton, &option, painter, widget);
        });
    }
}

impl ItemConsts for NativeButton {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

declare_item_vtable! {
    fn slint_get_NativeButtonVTable() -> NativeButtonVTable for NativeButton
}
