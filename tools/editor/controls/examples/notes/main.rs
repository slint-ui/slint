// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

slint::include_modules!();

const NOTEBOOKS: [&str; 3] = ["Personal", "Work", "Ideas"];

#[derive(Clone, Serialize, Deserialize)]
struct StoredNote {
    id: i32,
    title: String,
    body: String,
    notebook: usize,
    pinned: bool,
    archived: bool,
    updated: u64,
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    notes: Vec<StoredNote>,
}

struct Store {
    notes: Vec<StoredNote>,
    selected: Option<i32>,
    path: PathBuf,
    dirty: bool,
    load_error: Option<String>,
}

impl Store {
    fn open(path: PathBuf) -> Self {
        let loaded = fs::read(&path);
        let (notes, dirty, load_error) = match loaded {
            Ok(bytes) => match serde_json::from_slice::<Document>(&bytes) {
                Ok(document)
                    if document.version == 1
                        && document.notes.iter().all(|note| {
                            note.id > 0
                                && note.notebook < NOTEBOOKS.len()
                                && document.notes.iter().filter(|other| other.id == note.id).count()
                                    == 1
                        }) =>
                {
                    (document.notes, false, None)
                }
                Ok(_) => (
                    vec![],
                    false,
                    Some("The data file has an unsupported version or invalid note records".into()),
                ),
                Err(error) => (vec![], false, Some(format!("Cannot parse the data file: {error}"))),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (seed_notes(), true, None)
            }
            Err(error) => (vec![], false, Some(format!("Cannot read the data file: {error}"))),
        };
        Self { notes, selected: None, path, dirty, load_error }
    }

    fn save(&mut self) -> Result<(), String> {
        if let Some(error) = &self.load_error {
            return Err(format!(
                "Read-only: {error}. Original file preserved. Repair it or choose another data file."
            ));
        }
        if !self.dirty {
            return Ok(());
        }
        let document = Document { version: 1, notes: self.notes.clone() };
        let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
        atomic_write(&self.path, &bytes).map_err(|error| {
            format!("Cannot save {}: {error}. Edits remain in memory.", self.path.display())
        })?;
        self.dirty = false;
        Ok(())
    }
}

struct Application {
    ui: slint::Weak<NotesApp>,
    store: RefCell<Store>,
    model: Rc<VecModel<Note>>,
    autosave: Timer,
    export_directory: PathBuf,
}

impl Application {
    fn refresh(&self, update_editor: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<NotesState>();
        let query = state.get_query().to_lowercase();
        let view = state.get_view_index();
        let mut store = self.store.borrow_mut();
        let mut visible: Vec<_> = store
            .notes
            .iter()
            .filter(|note| {
                (match view {
                    1 => note.pinned && !note.archived,
                    2 => note.archived,
                    _ => !note.archived,
                }) && (query.is_empty()
                    || note.title.to_lowercase().contains(&query)
                    || note.body.to_lowercase().contains(&query)
                    || NOTEBOOKS[note.notebook].to_lowercase().contains(&query))
            })
            .cloned()
            .collect();
        visible.sort_by_key(|note| {
            (
                std::cmp::Reverse(note.pinned),
                std::cmp::Reverse(note.updated),
                std::cmp::Reverse(note.id),
            )
        });
        let previous_selection = store.selected;
        if !visible.iter().any(|note| Some(note.id) == store.selected) {
            store.selected = visible.first().map(|note| note.id);
        }
        self.model.set_vec(visible.iter().map(to_ui_note).collect::<Vec<_>>());
        state.set_note_count(store.notes.len() as i32);
        state.set_has_selection(store.selected.is_some());
        state.set_selected_id(store.selected.unwrap_or(-1));
        let selected = store.notes.iter().find(|note| Some(note.id) == store.selected);
        state
            .set_word_count(selected.map_or(0, |note| note.body.split_whitespace().count() as i32));
        if update_editor || previous_selection != store.selected {
            state.set_note_title(selected.map_or("", |note| note.title.as_str()).into());
            state.set_note_body(selected.map_or("", |note| note.body.as_str()).into());
            state.set_notebook_index(selected.map_or(0, |note| note.notebook as i32));
            state.set_note_pinned(selected.is_some_and(|note| note.pinned));
            state.set_note_archived(selected.is_some_and(|note| note.archived));
        }
    }

    fn status(&self, message: impl Into<slint::SharedString>) {
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<NotesState>().set_status(message.into());
        }
    }

    fn schedule_save(self: &Rc<Self>) {
        self.status("Unsaved changes");
        let weak = Rc::downgrade(self);
        self.autosave.start(TimerMode::SingleShot, Duration::from_millis(600), move || {
            if let Some(app) = weak.upgrade() {
                app.save();
            }
        });
    }

    fn save(&self) -> bool {
        let result = self.store.borrow_mut().save();
        match result {
            Ok(()) => {
                self.status("All changes saved");
                true
            }
            Err(error) => {
                self.status(error);
                false
            }
        }
    }

    fn can_edit(&self) -> bool {
        self.store.borrow().load_error.is_none()
            && self.ui.upgrade().is_some_and(|ui| !ui.global::<NotesState>().get_read_only())
    }

    fn edit(self: &Rc<Self>) {
        if !self.can_edit() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<NotesState>();
        let mut store = self.store.borrow_mut();
        let selected = store.selected;
        let Some(note) = store.notes.iter_mut().find(|note| Some(note.id) == selected) else {
            return;
        };
        note.title = state.get_note_title().to_string();
        note.body = state.get_note_body().to_string();
        note.notebook = state.get_notebook_index().clamp(0, 2) as usize;
        note.pinned = state.get_note_pinned();
        note.archived = state.get_note_archived();
        note.updated = now();
        store.dirty = true;
        drop(store);
        self.refresh(false);
        self.schedule_save();
    }

    fn new_note(self: &Rc<Self>) {
        if !self.can_edit() {
            return;
        }
        let mut store = self.store.borrow_mut();
        let Some(id) = store.notes.iter().map(|note| note.id).max().unwrap_or(0).checked_add(1)
        else {
            drop(store);
            self.status("Cannot create a note: no note IDs remain");
            return;
        };
        store.notes.push(StoredNote {
            id,
            title: "Untitled note".into(),
            body: String::new(),
            notebook: 0,
            pinned: false,
            archived: false,
            updated: now(),
        });
        store.selected = Some(id);
        store.dirty = true;
        drop(store);
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<NotesState>().set_query("".into());
            ui.global::<NotesState>().set_view_index(0);
        }
        self.refresh(true);
        self.schedule_save();
    }

    fn archive(self: &Rc<Self>) {
        if !self.can_edit() {
            return;
        }
        let mut store = self.store.borrow_mut();
        let selected = store.selected;
        if let Some(note) = store.notes.iter_mut().find(|note| Some(note.id) == selected) {
            note.archived = !note.archived;
            note.updated = now();
            store.dirty = true;
        }
        drop(store);
        self.refresh(true);
        self.schedule_save();
    }

    fn delete(self: &Rc<Self>) {
        if !self.can_edit() {
            return;
        }
        let mut store = self.store.borrow_mut();
        let selected = store.selected;
        store.notes.retain(|note| Some(note.id) != selected);
        store.dirty = true;
        drop(store);
        self.refresh(true);
        self.schedule_save();
    }

    fn export(&self) {
        let selected = {
            let store = self.store.borrow();
            store.notes.iter().find(|note| Some(note.id) == store.selected).cloned()
        };
        let Some(note) = selected else { return };
        let result = (|| -> std::io::Result<PathBuf> {
            fs::create_dir_all(&self.export_directory)?;
            let directory = self.export_directory.canonicalize()?;
            let slug: String = note
                .title
                .chars()
                .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
                .take(60)
                .collect();
            let slug = slug.trim_matches('-');
            let slug = if slug.is_empty() { "note" } else { slug };
            let path = directory.join(format!("{slug}-{}-{}.txt", note.id, nonce()));
            let mut file = OpenOptions::new().create_new(true).write(true).open(&path)?;
            write!(file, "{}\n\n{}\n", note.title, note.body)?;
            file.sync_all()?;
            Ok(path)
        })();
        match result {
            Ok(path) => self.status(format!("Exported to {}", path.display())),
            Err(error) => self.status(format!("Cannot export note: {error}")),
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn nonce() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
}

fn to_ui_note(note: &StoredNote) -> Note {
    let seconds = now().saturating_sub(note.updated);
    let updated = if seconds < 60 {
        "Just now".into()
    } else if seconds < 3600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{} h ago", seconds / 3600)
    } else {
        format!("{} d ago", seconds / 86400)
    };
    Note {
        id: note.id,
        title: if note.title.trim().is_empty() { "Untitled note" } else { &note.title }.into(),
        body: note.body.clone().into(),
        preview: note.body.split_whitespace().take(20).collect::<Vec<_>>().join(" ").into(),
        notebook: NOTEBOOKS[note.notebook].into(),
        updated: updated.into(),
        pinned: note.pinned,
        archived: note.archived,
    }
}

fn seed_notes() -> Vec<StoredNote> {
    [
        ("A little space to think", "Welcome to Fieldnotes.\n\nCapture an idea, keep a useful detail, or work through a thought. This space is yours.\n\nTry changing the notebook, pinning this note, or adjusting the reading settings. Your edits save automatically.\n\nStart small. One sentence is enough.", 0, true),
        ("Monday, with intention", "Three things to make room for this week:\n\n- Finish the small things before starting new ones.\n- Take a proper lunch break.\n- Leave a little time for something unexpected.\n\nA good week doesn't have to be a busy one.", 0, false),
        ("Design review", "What should the next version feel like?\n\nClearer choices. Fewer distractions. A useful default, with room to make it your own.\n\nQuestions for the team:\n- Is the main action easy to find?\n- Can the controls be used with a keyboard?\n- Does every setting earn its place?", 1, true),
        ("Small ideas, worth keeping", "A walking route that follows the river.\nA shelf for books that are still being read.\nA notebook that starts with a question instead of a date.\n\nLeave these here. See which one returns.", 2, false),
        ("Last season", "A few things that worked:\n\nKeep the morning clear. Write things down. Take the long way home when you can.\n\nArchived notes stay searchable in the Archive view.", 0, false),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (title, body, notebook, pinned))| StoredNote {
        id: index as i32 + 1,
        title: title.into(),
        body: body.into(),
        notebook,
        pinned,
        archived: index == 4,
        updated: now().saturating_sub(index as u64 * 3600),
    })
    .collect()
}

fn data_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("FIELDNOTES_DATA") {
        return Ok(PathBuf::from(path));
    }
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })
    };
    base.map(|base| base.join("Fieldnotes/notes.json")).ok_or_else(|| {
        "Cannot locate a data directory. Set FIELDNOTES_DATA to a JSON file path.".into()
    })
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent =
        path.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".fieldnotes-{}-{}.tmp", std::process::id(), nonce()));
    let result = (|| {
        let mut file = OpenOptions::new().create_new(true).write(true).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    }

    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    // Windows rename cannot replace an existing destination; MoveFileExW preserves it on failure.
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 1 | 8) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut path = None;
    let mut export_directory = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--data-file") => {
                path = Some(PathBuf::from(arguments.next().ok_or("--data-file requires a path")?));
            }
            Some("--export-directory") => {
                export_directory = Some(PathBuf::from(
                    arguments.next().ok_or("--export-directory requires a path")?,
                ));
            }
            Some("--help" | "-h") => {
                println!("Fieldnotes\n\nUsage: fieldnotes [--data-file PATH] [--export-directory PATH]\n\nFIELDNOTES_DATA overrides the default notes.json location.");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {}", argument.to_string_lossy()).into()),
        }
    }
    let path = match path {
        Some(path) => path,
        None => data_path()?,
    };
    let export_directory = export_directory.unwrap_or_else(|| {
        path.parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .join("exports")
    });
    let ui = NotesApp::new()?;
    let model = Rc::new(VecModel::default());
    ui.global::<NotesState>().set_notes(model.clone().into());
    let app = Rc::new(Application {
        ui: ui.as_weak(),
        store: RefCell::new(Store::open(path)),
        model,
        autosave: Timer::default(),
        export_directory,
    });
    let storage_read_only = app.store.borrow().load_error.is_some();
    ui.global::<NotesState>().set_storage_read_only(storage_read_only);
    if storage_read_only {
        ui.global::<NotesState>().set_read_only(true);
    }
    app.refresh(true);
    app.save();
    let state = ui.global::<NotesState>();
    state.on_new_note({
        let app = app.clone();
        move || app.new_note()
    });
    state.on_edit_note({
        let app = app.clone();
        move || app.edit()
    });
    state.on_archive_note({
        let app = app.clone();
        move || app.archive()
    });
    state.on_delete_note({
        let app = app.clone();
        move || app.delete()
    });
    state.on_save({
        let app = app.clone();
        move || {
            app.autosave.stop();
            app.save();
        }
    });
    state.on_export_note({
        let app = app.clone();
        move || app.export()
    });
    state.on_select_note({
        let app = app.clone();
        move |id| {
            app.store.borrow_mut().selected = Some(id);
            app.refresh(true);
        }
    });
    state.on_search({
        let app = app.clone();
        move |query| {
            if let Some(ui) = app.ui.upgrade() {
                ui.global::<NotesState>().set_query(query);
            }
            app.refresh(true);
        }
    });
    state.on_set_view({
        let app = app.clone();
        move |view| {
            if let Some(ui) = app.ui.upgrade() {
                ui.global::<NotesState>().set_view_index(view);
            }
            app.refresh(true);
        }
    });
    ui.window().on_close_requested({
        let app = app.clone();
        move || {
            app.autosave.stop();
            let dirty = app.store.borrow().dirty;
            if !dirty || app.save() {
                slint::CloseRequestResponse::HideWindow
            } else {
                slint::CloseRequestResponse::KeepWindowShown
            }
        }
    });
    ui.run()?;
    Ok(())
}
