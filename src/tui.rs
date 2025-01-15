//! # Text-based user interface (TUI)
//!
//! Utility functions for driving the text-based user interface (TUI) for
//! the Quartiles puzzle solver. These features really belong in the official
//! Ratatui library, as every application that uses Ratatui will need to
//! initialize and restore the terminal in the same way. But currently it
//! remains a responsibility of the application to do so.

use std::{io::{self, stdout, Stdout}, panic, sync::{Arc, Mutex}, thread};

use crossterm::{
	execute,
	terminal::{
		disable_raw_mode, enable_raw_mode,
		EnterAlternateScreen, LeaveAlternateScreen
	}
};
use ratatui::{backend::{Backend, CrosstermBackend}, Terminal};

////////////////////////////////////////////////////////////////////////////////
//                         Text-based user interface.                         //
////////////////////////////////////////////////////////////////////////////////

/// The text-based user interface (TUI) type.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Open the text-based user interface (TUI). Arrange for the terminal to be
/// restored to its original state in case of panic _on the calling thread
/// only_. During this call, the calling thread is the UI thread, by definition.
///
/// # Arguments
///
/// * `f` - The function to apply to the TUI.
///
/// # Returns
///
/// The result of applying `f` to the TUI.
///
/// # Errors
///
/// Any error that occurs while driving the TUI.
pub fn tui<F, T>(f: F) -> io::Result<T>
	where F: FnOnce(&mut Tui) -> io::Result<T>
{
	// Capture the original panic hook and replace it with one that restores
	// the terminal before panicking. The panic hook is a global resource, so we
	// use the Arc<Mutex<Option>> idiom to share it between the TUI thread and
	// the panic hook.
	let original_hook = panic::take_hook();
	let original_hook = Arc::new(Mutex::new(Some(original_hook)));
	let original_hook_clone = Arc::clone(&original_hook);
	let tui_thread = thread::current().id();
	panic::set_hook(Box::new(move |info| {
		if thread::current().id() == tui_thread
		{
			// Only restore the terminal if the panic occurred in the TUI
			// thread. We don't care about the result, because there isn't much
			// we can do to recover anyway, especially given that we are already
			// panicking.
			let _ = tui_restore();
		}
		// Call the original panic hook. Take care not to vacate the inner
		// Option, because we don't know enough about the semantics of the
		// original hook to decide that it should only be called once. We assume
		// that the original hook has multiple-call semantics, or that it guards
		// against being called multiple times within its own implementation.
		let original_hook = original_hook.lock().unwrap();
		original_hook.as_ref().unwrap()(info);
	}));
	// `tui_init` is non-atomic, so we must ensure that the terminal is restored
	// in the event of partial success.
	let result = match tui_init()
	{
		Ok(mut terminal) => f(&mut terminal),
		Err(e) => Err(e)
	};
	// We don't want to re-enter `tui_restore` in the event of a panic, so we
	// restore the original panic hook before calling it.
	panic::set_hook(original_hook_clone.lock().unwrap().take().unwrap());
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
