// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::VecDeque;
use std::io::{Stdout, stdout};
use std::time::Duration;

use anyhow::{Context as _, Result};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures_util::StreamExt as _;
use i_slint_springboard::{Device, DeviceId, DeviceStatus, LogLevel, SessionEvent};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::LaunchOptions;
use crate::session_driver::ProjectSessionController;

#[cfg(test)]
use crate::session_driver::{LOCAL_VIEWER_DEVICE_ID, RUST_APPLICATION_DEVICE_ID};

const MAX_LOG_LINES: usize = 200;

type SpringboardTerminal = Terminal<CrosstermBackend<Stdout>>;

pub async fn run(mut controller: ProjectSessionController, launch: LaunchOptions) -> Result<()> {
    let mut app = TuiApp::default();
    apply_startup_action(&mut controller, &mut app, &launch).await;
    app.absorb(controller.take_events());

    let mut terminal = initialize_terminal()?;
    let result = run_loop(&mut terminal, &mut controller, &mut app).await;
    let shutdown_result = controller.shutdown().await;
    let restore_result = restore_terminal(&mut terminal);

    result.and(shutdown_result).and(restore_result)
}

async fn apply_startup_action(
    controller: &mut ProjectSessionController,
    app: &mut TuiApp,
    launch: &LaunchOptions,
) {
    let requested = if let Some(device_id) = &launch.device {
        match DeviceId::new(device_id) {
            Ok(device_id) => Some(device_id),
            Err(error) => {
                app.error(error.to_string());
                None
            }
        }
    } else if launch.last {
        match controller.last_used_device().cloned() {
            Some(device_id) => Some(device_id),
            None => {
                app.error("No last-used device is available");
                None
            }
        }
    } else if launch.ios {
        match controller.preferred_ios_simulator() {
            Ok(device_id) => Some(device_id),
            Err(error) => {
                app.error(error.to_string());
                None
            }
        }
    } else if launch.android {
        app.error("Android emulator management is not available yet");
        None
    } else {
        None
    };

    if let Some(device_id) = requested
        && let Err(error) = controller.launch(&device_id).await
    {
        app.error(error.to_string());
    }
}

async fn run_loop(
    terminal: &mut SpringboardTerminal,
    controller: &mut ProjectSessionController,
    app: &mut TuiApp,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    loop {
        controller.poll().await?;
        app.absorb(controller.take_events());
        terminal.draw(|frame| render(frame, controller, app))?;
        if app.should_quit {
            return Ok(());
        }

        tokio::select! {
            event = events.next() => {
                match event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        handle_key(key, controller, app).await;
                    }
                    Some(Err(error)) => app.error(format!("Terminal input failed: {error}")),
                    None => return Ok(()),
                    _ => {}
                }
            }
            _ = tick.tick() => {}
        }
    }
}

async fn handle_key(key: KeyEvent, controller: &mut ProjectSessionController, app: &mut TuiApp) {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }
    if app.manual_address.is_some() {
        match key.code {
            KeyCode::Esc => app.manual_address = None,
            KeyCode::Backspace => {
                app.manual_address.as_mut().unwrap().pop();
            }
            KeyCode::Enter => {
                let address = app.manual_address.take().unwrap();
                match controller.add_manual_device(&address) {
                    Ok(device_id) => {
                        if let Some(index) =
                            controller.session().devices().keys().position(|id| id == &device_id)
                        {
                            app.selected = index;
                        }
                    }
                    Err(error) => app.error(error.to_string()),
                }
            }
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.manual_address.as_mut().unwrap().push(character);
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Up => app.select_previous(controller.session().devices().len()),
        KeyCode::Down => app.select_next(controller.session().devices().len()),
        KeyCode::Enter => {
            if let Some(device_id) = app.selected_device(controller) {
                if let Err(error) = controller.launch(&device_id).await {
                    app.error(error.to_string());
                }
            }
        }
        KeyCode::Char('s') => {
            if let Some(device_id) = app.selected_device(controller)
                && let Err(error) = controller.stop(&device_id).await
            {
                app.error(error.to_string());
            }
        }
        KeyCode::Char('r') => {
            if let Some(device_id) = app.selected_device(controller)
                && let Err(error) = controller.refresh(&device_id)
            {
                app.error(error.to_string());
            }
        }
        KeyCode::Char('b') => {
            if let Some(device_id) = app.selected_device(controller)
                && let Err(error) = controller.rebuild(&device_id)
            {
                app.error(error.to_string());
            }
        }
        KeyCode::Char('a') => {
            app.manual_address = Some(String::new());
        }
        _ => {}
    }
}

fn initialize_terminal() -> Result<SpringboardTerminal> {
    enable_raw_mode().context("Failed to enable terminal raw mode")?;
    let mut output = stdout();
    if let Err(error) = execute!(output, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(error).context("Failed to enter the alternate terminal screen");
    }
    Terminal::new(CrosstermBackend::new(output)).context("Failed to initialize the terminal")
}

fn restore_terminal(terminal: &mut SpringboardTerminal) -> Result<()> {
    disable_raw_mode().context("Failed to disable terminal raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("Failed to leave the alternate terminal screen")?;
    terminal.show_cursor().context("Failed to restore the terminal cursor")
}

#[derive(Default)]
struct TuiApp {
    selected: usize,
    logs: VecDeque<String>,
    manual_address: Option<String>,
    should_quit: bool,
}

impl TuiApp {
    fn absorb(&mut self, events: Vec<SessionEvent>) {
        for event in events {
            match event {
                SessionEvent::Log { level, message, .. } => {
                    self.push_log(format!("{} {message}", log_level_label(level)));
                }
                SessionEvent::Diagnostic { severity, message, .. } => {
                    self.push_log(format!("{severity:?} {message}"));
                }
                SessionEvent::Error { message, .. } => self.error(message),
                SessionEvent::DeviceChanged { .. }
                | SessionEvent::DeviceRemoved { .. }
                | SessionEvent::ActiveDeviceChanged { .. }
                | SessionEvent::LastUsedDeviceChanged { .. } => {}
            }
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        self.push_log(format!("ERROR {}", message.into()));
    }

    #[cfg(test)]
    fn info(&mut self, message: impl Into<String>) {
        self.push_log(format!("INFO {}", message.into()));
    }

    fn push_log(&mut self, line: String) {
        self.logs.push_back(line);
        while self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
    }

    fn select_previous(&mut self, device_count: usize) {
        if device_count == 0 {
            self.selected = 0;
        } else if self.selected == 0 {
            self.selected = device_count - 1;
        } else {
            self.selected -= 1;
        }
    }

    fn select_next(&mut self, device_count: usize) {
        if device_count == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1) % device_count;
        }
    }

    fn selected_device(&self, controller: &ProjectSessionController) -> Option<DeviceId> {
        controller.session().devices().keys().nth(self.selected).cloned()
    }
}

fn render(frame: &mut ratatui::Frame<'_>, controller: &ProjectSessionController, app: &TuiApp) {
    let view = ViewState {
        project: controller.session().project().project_root.display().to_string(),
        devices: controller.session().devices().values().collect(),
        active: controller.session().active_device(),
        last_used: controller.last_used_device(),
        selected: app.selected,
        logs: app.logs.iter().map(String::as_str).collect(),
        manual_address: app.manual_address.as_deref(),
    };
    render_view(frame, &view);
}

struct ViewState<'a> {
    project: String,
    devices: Vec<&'a Device>,
    active: Option<&'a DeviceId>,
    last_used: Option<&'a DeviceId>,
    selected: usize,
    logs: Vec<&'a str>,
    manual_address: Option<&'a str>,
}

fn render_view(frame: &mut ratatui::Frame<'_>, view: &ViewState<'_>) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let last_used = view.last_used.map(DeviceId::as_str).unwrap_or("None");
    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Server: Running",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!("Project: {}  Last used: {last_used}", view.project)),
    ])
    .block(Block::default().title("Slint Springboard").borders(Borders::ALL));
    frame.render_widget(header, areas[0]);

    let devices_block = Block::default().title("Devices").borders(Borders::ALL);
    if view.devices.is_empty() {
        frame.render_widget(
            Paragraph::new("No devices are available.").block(devices_block),
            areas[1],
        );
    } else {
        let items = view
            .devices
            .iter()
            .map(|device| {
                let active = (view.active == Some(&device.id)).then_some("●").unwrap_or(" ");
                let last = (view.last_used == Some(&device.id)).then_some("★").unwrap_or(" ");
                let version = device.version.as_deref().map(|version| format!(" v{version}"));
                ListItem::new(format!(
                    "{active} {last} {:<20} {:<16}{}",
                    device.name,
                    status_label(device),
                    version.as_deref().unwrap_or_default()
                ))
            })
            .collect::<Vec<_>>();
        let list = List::new(items)
            .block(devices_block)
            .highlight_symbol("▶ ")
            .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let mut state = ListState::default()
            .with_selected(Some(view.selected.min(view.devices.len().saturating_sub(1))));
        frame.render_stateful_widget(list, areas[1], &mut state);
    }

    let logs = if view.logs.is_empty() {
        vec![Line::from("No session output yet.")]
    } else {
        view.logs.iter().map(|line| Line::from((*line).to_string())).collect()
    };
    frame.render_widget(
        Paragraph::new(logs)
            .wrap(Wrap { trim: false })
            .block(Block::default().title("Session Log").borders(Borders::ALL)),
        areas[2],
    );

    let help = app_footer(view);
    frame.render_widget(
        Paragraph::new(help).block(Block::default().borders(Borders::ALL)),
        areas[3],
    );
}

fn app_footer(view: &ViewState<'_>) -> String {
    view.manual_address.map_or_else(
        || "↑/↓ Select  Enter Launch  s Stop  r Refresh  b Rebuild  a Add remote  q Quit".into(),
        |address| format!("Manual viewer address (host:port): {address}_  Enter Add  Esc Cancel"),
    )
}

fn status_label(device: &Device) -> String {
    match &device.status {
        DeviceStatus::Available => "Available".into(),
        DeviceStatus::Unavailable
            if device.kind == i_slint_springboard::DeviceKind::RemoteViewer =>
        {
            "Offline".into()
        }
        DeviceStatus::Unavailable => "Unavailable".into(),
        DeviceStatus::Resolving => "Resolving".into(),
        DeviceStatus::Starting => "Starting".into(),
        DeviceStatus::Booting => "Booting simulator".into(),
        DeviceStatus::Connecting => "Connecting".into(),
        DeviceStatus::Reconnecting => "Reconnecting".into(),
        DeviceStatus::Downloading { bytes_received, total_bytes } => total_bytes.map_or_else(
            || format!("Downloading viewer: {} MiB", bytes_received / 1024 / 1024),
            |total| {
                let percent = bytes_received.saturating_mul(100) / total.max(1);
                format!("Downloading viewer: {percent}%")
            },
        ),
        DeviceStatus::Installing => "Installing viewer".into(),
        DeviceStatus::Compiling => "Compiling".into(),
        DeviceStatus::Reloading => "Reloading".into(),
        DeviceStatus::Rebuilding => "Rebuilding".into(),
        DeviceStatus::Running => "Running".into(),
        DeviceStatus::RunningWithError { message } => format!("Running with error: {message}"),
        DeviceStatus::Stopping => "Stopping".into(),
        DeviceStatus::Failed { message } => format!("Failed: {message}"),
        DeviceStatus::Incompatible { installed, required } => {
            format!("Protocol mismatch: Slint {installed}, needs {required}")
        }
    }
}

fn log_level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warning => "WARN ",
        LogLevel::Information => "INFO ",
        LogLevel::Debug => "DEBUG",
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;

    use super::*;
    use i_slint_springboard::{DeviceCapabilities, DeviceKind, DeviceOrigin};

    fn device(status: DeviceStatus) -> Device {
        Device {
            id: DeviceId::new(LOCAL_VIEWER_DEVICE_ID).unwrap(),
            name: "Local Viewer".into(),
            kind: DeviceKind::LocalViewer,
            origin: DeviceOrigin::BuiltIn,
            status,
            capabilities: DeviceCapabilities::launchable(),
            version: Some("1.18.0".into()),
            platform: Some(std::env::consts::OS.into()),
        }
    }

    fn remote_device(status: DeviceStatus) -> Device {
        Device {
            id: DeviceId::new("remote:phone").unwrap(),
            name: "Nigel's iPhone".into(),
            kind: DeviceKind::RemoteViewer,
            origin: DeviceOrigin::Remembered,
            status,
            capabilities: DeviceCapabilities::launchable(),
            version: Some("1.17.2".into()),
            platform: Some("ios".into()),
        }
    }

    fn snapshot(
        devices: &[Device],
        active: Option<&DeviceId>,
        last_used: Option<&DeviceId>,
        logs: &[&str],
    ) -> String {
        let backend = TestBackend::new(88, 22);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render_view(
                    frame,
                    &ViewState {
                        project: "/project".into(),
                        devices: devices.iter().collect(),
                        active,
                        last_used,
                        selected: 0,
                        logs: logs.to_vec(),
                        manual_address: None,
                    },
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                let line =
                    (0..buffer.area.width).map(|x| buffer[(x, y)].symbol()).collect::<String>();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn empty_device_snapshot() {
        let rendered = snapshot(&[], None, None, &[]);
        assert!(rendered.contains("No devices are available."));
        assert!(rendered.contains("Last used: None"));
        assert!(rendered.contains("No session output yet."));
    }

    #[test]
    fn running_device_snapshot() {
        let target = device(DeviceStatus::Running);
        let rendered = snapshot(
            std::slice::from_ref(&target),
            Some(&target.id),
            Some(&target.id),
            &["INFO  Local viewer started"],
        );
        assert!(rendered.contains("● ★ Local Viewer"));
        assert!(rendered.contains("Running"));
        assert!(rendered.contains("INFO  Local viewer started"));
    }

    #[test]
    fn failed_device_snapshot() {
        let target = device(DeviceStatus::Failed { message: "viewer exited".into() });
        let rendered = snapshot(
            std::slice::from_ref(&target),
            None,
            Some(&target.id),
            &["ERROR Local viewer exited unexpectedly"],
        );
        assert!(rendered.contains("Failed: viewer exited"));
        assert!(rendered.contains("ERROR Local viewer exited unexpectedly"));
    }

    #[test]
    fn viewer_download_snapshot_shows_progress() {
        let target = device(DeviceStatus::Downloading { bytes_received: 3, total_bytes: Some(4) });
        let rendered =
            snapshot(std::slice::from_ref(&target), Some(&target.id), Some(&target.id), &[]);

        assert!(rendered.contains("Downloading viewer: 75%"));
    }

    #[test]
    fn rust_rebuild_and_running_error_snapshots_are_distinct() {
        let mut target = device(DeviceStatus::Rebuilding);
        target.id = DeviceId::new(RUST_APPLICATION_DEVICE_ID).unwrap();
        target.name = "Rust Application (demo)".into();
        target.kind = DeviceKind::RustApplication;
        let rendered = snapshot(
            std::slice::from_ref(&target),
            Some(&target.id),
            Some(&target.id),
            &["INFO  The Slint Rust interface changed"],
        );
        assert!(rendered.contains("Rebuilding"));

        target.status = DeviceStatus::RunningWithError { message: "Cargo build failed".into() };
        let rendered =
            snapshot(std::slice::from_ref(&target), Some(&target.id), Some(&target.id), &[]);
        assert!(rendered.contains("Running with error: Cargo build failed"));
    }

    #[test]
    fn no_last_device_snapshot() {
        let target = device(DeviceStatus::Available);
        let rendered = snapshot(std::slice::from_ref(&target), None, None, &[]);
        assert!(rendered.contains("Last used: None"));
        assert!(rendered.contains("Available"));
        assert!(!rendered.contains("★ Local Viewer"));
    }

    #[test]
    fn log_buffer_is_bounded() {
        let mut app = TuiApp::default();
        for index in 0..MAX_LOG_LINES + 10 {
            app.info(index.to_string());
        }

        assert_eq!(app.logs.len(), MAX_LOG_LINES);
        assert_eq!(app.logs.front().map(String::as_str), Some("INFO 10"));
    }

    #[test]
    fn manual_address_entry_replaces_the_shortcut_footer() {
        let view = ViewState {
            project: "/project".into(),
            devices: Vec::new(),
            active: None,
            last_used: None,
            selected: 0,
            logs: Vec::new(),
            manual_address: Some("viewer.local:41000"),
        };

        assert_eq!(
            app_footer(&view),
            "Manual viewer address (host:port): viewer.local:41000_  Enter Add  Esc Cancel"
        );
    }

    #[test]
    fn remote_status_snapshots_distinguish_offline_reconnecting_and_incompatible() {
        let offline = remote_device(DeviceStatus::Unavailable);
        assert!(snapshot(&[offline], None, None, &[]).contains("Offline"));

        let reconnecting = remote_device(DeviceStatus::Reconnecting);
        let rendered = snapshot(
            std::slice::from_ref(&reconnecting),
            Some(&reconnecting.id),
            Some(&reconnecting.id),
            &[],
        );
        assert!(rendered.contains("Reconnecting"));

        let incompatible = remote_device(DeviceStatus::Incompatible {
            installed: "1.17.2".into(),
            required: "1.18.0".into(),
        });
        let rendered = snapshot(&[incompatible], None, None, &[]);
        assert!(rendered.contains("Protocol mismatch: Slint 1.17.2, needs 1.18.0"));
    }
}
