//! Native file picker for loading ROMs.
//!
//! Uses the OS-native file dialog (Win32 `IFileOpenDialog`,
//! macOS `NSOpenPanel`, Linux XDG Desktop Portal / GTK3) via the
//! `rfd` crate.

use std::path::PathBuf;

/// Opens the OS-native file picker filtered to NES ROMs.
///
/// Blocks until the user picks a file or cancels. Returns `None`
/// on cancel.
pub(super) fn pick_rom() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open NES ROM")
        .add_filter("NES ROMs", &["nes"])
        .add_filter("All files", &["*"])
        .pick_file()
}
