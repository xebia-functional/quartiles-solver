//! # Text-based user interface (TUI)
//!
//! Utility functions for driving the text-based user interface (TUI) for
//! the Quartiles puzzle solver. These features really belong in the official
//! Ratatui library, as every application that uses Ratatui will need to
//! initialize and restore the terminal in the same way. But currently it
//! remains a responsibility of the application to do so.

use std::{io::{self, stdout, Stdout}, panic};

use crossterm::{
	execute,
	terminal::{
		disable_raw_mode, enable_raw_mode,
		EnterAlternateScreen, LeaveAlternateScreen
	}
};
use quartiles_solver::dictionary::Dictionary;
use ratatui::{backend::{Backend, CrosstermBackend}, Terminal};

use crate::app::App;

////////////////////////////////////////////////////////////////////////////////
//                         Text-based user interface.                         //
////////////////////////////////////////////////////////////////////////////////

/// The text-based user interface (TUI) type.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Open the text-based user interface (TUI) for inputting and solving a
/// Quartiles puzzle. Arrange for the terminal to be restored to its original
/// state in case of panic.
///
/// # Arguments
///
/// * `highlight_duration_µs` - How long (in µs) to highlight an individual
///   word in the TUI.
/// * `dictionary` - The dictionary to use for solving the puzzle.
///
/// # Returns
///
/// The solution to the puzzle, as a word list.
///
/// # Errors
///
/// Any error that occurs while driving the TUI.
pub fn tui(highlight_duration_µs: u64, dictionary: Dictionary) -> io::Result<Vec<String>>
{
	// Capture the original panic hook and replace it with one that restores
	// the terminal before panicking.
	let original_hook = panic::take_hook();
	let mut tui = tui_init()?;
	panic::set_hook(Box::new(move |info| {
		let _ = tui_restore();
		original_hook(info);
	}));
	let result = App::new(highlight_duration_µs, dictionary).run(&mut tui);
	tui_restore()?;
	result
}

/// Initialize the text-based user interface (TUI).
///
/// # Returns
///
/// The initialized TUI.
///
/// # Errors
///
/// Any error that occurs while initializing the TUI.
fn tui_init() -> io::Result<Tui>
{
	let mut stdout = stdout();
	execute!(stdout, EnterAlternateScreen)?;
	enable_raw_mode()?;
	Terminal::new(CrosstermBackend::new(stdout))
}

/// Restore the terminal to its original state.
///
/// # Errors
///
/// Any error that occurs while restoring the terminal.
fn tui_restore() -> io::Result<()>
{
	let mut stdout = stdout();
	execute!(stdout, LeaveAlternateScreen)?;
	disable_raw_mode()?;
	// Take care to restore the cursor.
	CrosstermBackend::new(stdout).show_cursor()
}
