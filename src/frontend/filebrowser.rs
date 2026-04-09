//! Minimal in-emulator file browser (F3) built with raygui.

use std::path::{Path, PathBuf};

use raylib::prelude::*;

/// A simple file/directory browser for loading ROMs.
pub(super) struct FileBrowser {
    visible: bool,
    dir: PathBuf,
    entries: Vec<Entry>,
    scroll_index: i32,
    active_index: i32,
    /// Set when the user picks a file.
    picked: Option<PathBuf>,
}

struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

impl FileBrowser {
    pub(super) fn new() -> Self {
        let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut browser = Self {
            visible: false,
            dir: PathBuf::new(),
            entries: Vec::new(),
            scroll_index: 0,
            active_index: -1,
            picked: None,
        };
        browser.navigate(&dir);
        browser
    }

    pub(super) fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns the path of a picked ROM, if any.
    pub(super) fn take_picked(&mut self) -> Option<PathBuf> {
        self.picked.take()
    }

    /// Draws the browser panel. Call once per frame.
    pub(super) fn draw(&mut self, draw: &mut RaylibDrawHandle<'_>) {
        if !self.visible {
            return;
        }

        let px = 10;
        let py = 30;
        let pw = 500;
        let ph = 400;
        let pad = 8;

        // Background
        draw.draw_rectangle(px, py, pw, ph, Color::new(20, 20, 20, 240));
        draw.draw_rectangle_lines(px, py, pw, ph, Color::RAYWHITE);

        // Title with current path
        let title = format!("Open ROM  [{}]", self.dir.display());
        let _ = draw.gui_label(
            Rectangle::new(
                (px + pad) as f32,
                (py + pad) as f32,
                (pw - pad * 2) as f32,
                20.0,
            ),
            &title,
        );

        // File list
        let list_y = (py + 34) as f32;
        let list_h = (ph - 78) as f32;
        let items: String = self
            .entries
            .iter()
            .map(|e| {
                if e.is_dir {
                    format!("[{}]", e.name)
                } else {
                    e.name.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(";");

        let _ = draw.gui_list_view(
            Rectangle::new((px + pad) as f32, list_y, (pw - pad * 2) as f32, list_h),
            &items,
            &mut self.scroll_index,
            &mut self.active_index,
        );

        // Buttons row
        let btn_y = (py + ph - 36) as f32;
        let btn_w = 80.0;

        // "Up" button — navigate to parent
        if draw.gui_button(Rectangle::new((px + pad) as f32, btn_y, btn_w, 28.0), "Up")
            && let Some(parent) = self.dir.parent().map(Path::to_path_buf)
        {
            self.navigate(&parent);
        }

        // "Open" button — load selected entry
        if draw.gui_button(
            Rectangle::new((px + pad + 90) as f32, btn_y, btn_w, 28.0),
            "Open",
        ) {
            self.confirm_selection();
        }

        // "Cancel" button
        if draw.gui_button(
            Rectangle::new((px + pad + 180) as f32, btn_y, btn_w, 28.0),
            "Cancel",
        ) {
            self.visible = false;
        }

        // Enter key confirms
        if draw.is_key_pressed(KeyboardKey::KEY_ENTER) {
            self.confirm_selection();
        }
    }

    fn confirm_selection(&mut self) {
        let idx = self.active_index;
        if idx < 0 {
            return;
        }
        let Some(entry) = self.entries.get(idx as usize) else {
            return;
        };
        if entry.is_dir {
            let path = entry.path.clone();
            self.navigate(&path);
        } else {
            self.picked = Some(entry.path.clone());
            self.visible = false;
        }
    }

    fn navigate(&mut self, dir: &Path) {
        self.dir = dir.to_path_buf();
        self.entries.clear();
        self.scroll_index = 0;
        self.active_index = -1;

        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();

            if name.starts_with('.') {
                continue; // skip hidden
            }

            if path.is_dir() {
                dirs.push(Entry {
                    name,
                    path,
                    is_dir: true,
                });
            } else if name.to_ascii_lowercase().ends_with(".nes") {
                files.push(Entry {
                    name,
                    path,
                    is_dir: false,
                });
            }
        }

        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));

        self.entries.extend(dirs);
        self.entries.extend(files);
    }
}
