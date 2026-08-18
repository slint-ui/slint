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
use crate::session_driver::LOCAL_VIEWER_DEVICE_ID;

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
        app.error("iOS Simulator management is not available yet");
        None
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
        controller.poll()?;
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
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
        KeyCode::Char('a') => {
            app.info("Manual remote devices will be available with remote viewer support");
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
                    status_label(&device.status),
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

    frame.render_widget(
        Paragraph::new("↑/↓ Select  Enter Launch  s Stop  r Refresh  a Add remote  q Quit")
            .block(Block::default().borders(Borders::ALL)),
        areas[3],
    );
}

fn status_label(status: &DeviceStatus) -> String {
    match status {
        DeviceStatus::Available => "Available".into(),
        DeviceStatus::Unavailable => "Unavailable".into(),
        DeviceStatus::Starting => "Starting".into(),
        DeviceStatus::Connecting => "Connecting".into(),
        DeviceStatus::Running => "Running".into(),
        DeviceStatus::Stopping => "Stopping".into(),
        DeviceStatus::Failed { message } => format!("Failed: {message}"),
        DeviceStatus::Incompatible { installed, required } => {
            format!("Incompatible: {installed}, needs {required}")
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
}
