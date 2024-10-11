//! # Application
//!
//! The application state and logic, including the text-based user interface
//! (TUI).

use std::{collections::HashSet, io, mem::swap, rc::Rc, time::{Duration, Instant}};

use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use fixedstr::str8;
use quartiles_solver::{dictionary::Dictionary, solver::{FragmentPath, Solver}};
use ratatui::{
	buffer::Buffer, layout::{Alignment, Constraint, Direction, Layout, Rect}, style::{Color, Style, Stylize}, text::{Line, Text}, widgets::{
		block::{Position, Title},
		Block, BorderType, Borders, List, ListState, Paragraph,
		StatefulWidget, Widget, Wrap
	}, Frame
};

use crate::tui::Tui;

////////////////////////////////////////////////////////////////////////////////
//                                Application.                                //
////////////////////////////////////////////////////////////////////////////////

/// The application state.
#[must_use]
pub struct App
{
	/// Whether the application is running.
	state: ExecutionState,

	/// How long (in µs) to highlight an individual word in the TUI.
	highlight_duration_µs: u64,

	/// The dictionary to use for solving the puzzle.
	dictionary: Rc<Dictionary>,

	/// The coordinates of the cursor. The first element is X, which
	/// corresponds to the column, and the second element is Y, which
	/// corresponds to the row. The origin is the top-left corner.
	cursor: (u8, u8),

	/// The content of the 4×5 grid, linearized in row-major order. The first
	/// element is the top-left corner (i.e., the origin), and the last element
	/// is the bottom-right corner.
	cells: [str8; 20]
}

// Public interface.
impl App
{
	/// Create a new application state.
	///
	/// # Arguments
	///
	/// * `highlight_duration_µs` - How long (in µs) to highlight an individual
	///   word in the TUI.
	/// * `dictionary` - The dictionary to use for solving the puzzle.
	///
	/// # Returns
	///
	/// The new application state.
	#[inline]
	pub fn new(highlight_duration_µs: u64, dictionary: Dictionary) -> Self
	{
		Self {
			state: ExecutionState::Populating,
			highlight_duration_µs,
			dictionary: Rc::new(dictionary),
			cursor: (0, 0),
			cells: [str8::default(); 20]
		}
	}

	/// Run the application. This amounts to:
	///
	/// * Running any background tasks, such as the solver or the highlighter.
	/// * Rendering the application frame.
	/// * Processing events.
	///
	/// # Arguments
	///
	/// * `tui` - The text-based user interface (TUI).
	///
	/// # Returns
	///
	/// The solution to the puzzle, as a word list.
	///
	/// # Errors
	///
	/// Any error that occurs while running the application.
	pub fn run(mut self, tui: &mut Tui) -> io::Result<Vec<String>>
	{
		while self.is_running()
		{
			self.process_systems();
			tui.draw(|frame| self.render_frame(frame))?;
			self.process_event()?;
		}
		// Only produce a solution if the solver has finished.
		match self.state
		{
			ExecutionState::Exiting { solution } => Ok(solution),
			_ => Ok(vec![])
		}
	}

	/// Check if the application is running.
	///
	/// # Returns
	///
	/// `true` if the application is running, `false` otherwise.
	#[inline]
	#[must_use]
	pub fn is_running(&self) -> bool
	{
		!matches!(self.state, ExecutionState::Exiting { .. })
	}
}

// Private implementation details.
impl App
{
	/// Move the cursor by the given deltas, saturating at the edges of the
	/// grid.
	///
	/// # Arguments
	///
	/// * `dx` - The change in the X-coordinate.
	/// * `dy` - The change in the Y-coordinate.
	fn move_cursor(&mut self, dx: i8, dy: i8)
	{
		let x = self.cursor.0 as i8 + dx;
		let y = self.cursor.1 as i8 + dy;
		if (0..4).contains(&x) && (0..5).contains(&y)
		{
			self.cursor = (x as u8, y as u8);
		}
	}

	/// Move the cursor by the given index delta, saturating at the edges of the
	/// grid. This supports tabbing through the cells.
	///
	/// # Arguments
	fn move_index(&mut self, di: i8)
	{
		let index = self.cursor.1 as usize * 4 + self.cursor.0 as usize;
		let new_index = index as i8 + di;
		if (0..20).contains(&new_index)
		{
			self.cursor = (new_index as u8 & 3, new_index as u8 >> 2);
		}
	}

	/// Get the index of the current cell.
	///
	/// # Returns
	///
	/// The index of the current cell.
	#[inline]
	#[must_use]
	fn current_index(&self) -> usize
	{
		self.cursor.1 as usize * 4 + self.cursor.0 as usize
	}

	/// Get the content of the current cell.
	///
	/// # Returns
	///
	/// The content of the current cell.
	#[inline]
	#[must_use]
	#[cfg(test)]
	fn current_cell(&self) -> &str8
	{
		&self.cells[self.current_index()]
	}

	/// Get a mutable reference to the content of the current cell.
	///
	/// # Returns
	///
	/// A mutable reference to the content of the current cell.
	#[inline]
	#[must_use]
	fn current_cell_mut(&mut self) -> &mut str8
	{
		&mut self.cells[self.current_index()]
	}

	/// Delete the last character of the current cell. If the cell is empty, do
	/// nothing.
	fn delete(&mut self)
	{
		let cell = self.current_cell_mut();
		cell.truncate(cell.len().saturating_sub(1));
	}

	/// Clear the content of the current cell.
	fn clear(&mut self)
	{
		let cell = self.current_cell_mut();
		cell.clear();
	}

	/// Clear the contents of all cells.
	fn clear_all(&mut self)
	{
		self.cells.iter_mut().for_each(str8::clear);
	}

	/// Move the word index. If nothing is highlighted, use the sign of the
	/// change to determine which end of the solution to start from, i.e.,
	/// positive for the beginning and negative for the end.
	///
	/// If the change would move the index out of bounds, remove the highlight.
	///
	/// # Arguments
	///
	/// * `di` - The change in the word index.
	fn move_word_index(&mut self, di: i8)
	{
		if let ExecutionState::Finished { ref solver, ref mut highlight, .. } = self.state
		{
			let solution = solver.solution();
			if let Some(index) = highlight
			{
				let new_highlight = index.wrapping_add(di as usize);
				if (0..solution.len()).contains(&new_highlight)
				{
					*highlight = Some(new_highlight);
				}
				else
				{
					*highlight = None;
				}
			}
			else if di > 0
			{
				*highlight = Some((di.wrapping_sub(1)) as usize);
			}
			else if di < 0
			{
				*highlight = Some(solution.len().wrapping_add(di as usize));
			}
		}
	}

	/// Append the given alphabetic character to the current cell. If the cell
	/// is full, do nothing.
	///
	/// # Arguments
	///
	/// * `c` - The character to append.
	///
	/// # Panics
	///
	/// If the character is not alphabetic.
	fn append(&mut self, c: char)
	{
		assert!(c.is_alphabetic());
		let cell = self.current_cell_mut();
		if cell.len() < 8
		{
			cell.push_char(c);
		}
	}

	/// Render the application frame.
	///
	/// # Arguments
	///
	/// * `frame` - The target frame.
	fn render_frame(&self, frame: &mut Frame)
	{
		frame.render_widget(self, frame.size());
	}

	/// Render the [population](ExecutionState::Populating) UI.
	///
	/// # Arguments
	///
	/// * `area` - The target area.
	/// * `buf` - The target buffer.
	fn render_populating(&self, area: Rect, buf: &mut Buffer)
	{
		// Split the screen into two parts: the puzzle and the solution.
		let outer = self.split_outer_screen(area);
		// The puzzle comprises a 4×5 grid of cells.
		let board = self.split_board(outer[0]);
		// Render the board.
		self.render_board(
			outer[0],
			buf,
			Some(
				"\
					←↑↓→ - move \
					⇥ - next \
					⇧⇥ - previous \
					A-Z - edit \
					⌫ - delete \
					⌦ - clear\
				".cyan()
			),
			Some("↵ – solve".green().bold())
		);
		// Render all of the cells.
		self.render_cells(board, buf, |index, cell| {
			let cell_style =
				if index == self.current_index()
				{
					Style::default()
						.fg(Color::Black)
						.bg(Color::Cyan)
				}
				else
				{
					Style::default()
				};
			let border_color =
				if cell.is_empty() { Color::Red }
				else { Color::White };
			let block = Block::new()
				.border_type(BorderType::Rounded)
				.borders(Borders::ALL)
				.border_style(Style::default().fg(border_color));
			let cell = Paragraph::new(cell.as_str())
				.block(block)
				.alignment(Alignment::Left)
				.style(cell_style)
				.wrap(Wrap { trim: true });
			cell
		});
		// Render the empty solution.
		self.render_solution_list(
			outer[1],
			buf,
			None,
			Some(None),
			None::<&str>,
			None,
			None
		);
	}

	/// Render the [solving](ExecutionState::Solving) UI.
	///
	/// # Arguments
	///
	/// * `area` - The target area.
	/// * `buf` - The target buffer.
	/// * `solver` - The solver.
	fn render_solving(&self, area: Rect, buf: &mut Buffer, solver: &Solver)
	{
		// Split the screen into two parts: the puzzle and the solution.
		let outer = self.split_outer_screen(area);
		// The puzzle comprises a 4×5 grid of cells.
		let board = self.split_board(outer[0]);
		// Render the board.
		self.render_board(outer[0], buf, None::<&str>, None::<&str>);
		// Render all of the cells.
		self.render_cells(board, buf, |_, cell| {
			let block = Block::new()
					.border_type(BorderType::Rounded)
					.borders(Borders::ALL)
					.border_style(Style::default().fg(Color::White));
				let cell = Paragraph::new(cell.as_str())
					.block(block)
					.alignment(Alignment::Left)
					.style(Style::default())
					.wrap(Wrap { trim: true });
				cell
		});
		// Render the solution.
		self.render_solution_list(
			outer[1],
			buf,
			Some(solver),
			None,
			None::<&str>,
			Some(Style::default().fg(Color::White)),
			None
		);
	}

	/// Render a [highlighting](ExecutionState::Highlighting) UI.
	///
	/// # Arguments
	///
	/// * `area` - The target area.
	/// * `buf` - The target buffer.
	/// * `solver` - The solver.
	/// * `path` - The fragment path of the solution to highlight.
	fn render_highlighting(
		&self,
		area: Rect,
		buf: &mut Buffer,
		solver: &Solver,
		path: &FragmentPath
	) {
		// Split the screen into two parts: the puzzle and the solution.
		let outer = self.split_outer_screen(area);
		// The puzzle comprises a 4×5 grid of cells.
		let board = self.split_board(outer[0]);
		self.render_board(outer[0], buf, None::<&str>, None::<&str>);
		// Build all of the cells.
		self.render_cells(board, buf, |index, cell| {
			let in_fragment = path.iter()
				.any(|i| matches!(i, Some(x) if x == index));
			let border_color =
				if in_fragment { Color::Black }
				else { Color::White };
			let block = Block::new()
				.border_type(BorderType::Rounded)
				.borders(Borders::ALL)
				.border_style(Style::default().fg(border_color));
			let cell =
				if in_fragment
				{
					let index_in_fragment = path.iter()
						.position(|i| matches!(i, Some(x) if x == index))
						.unwrap();
					let label = format!(
						"{} {}",
						index_in_fragment + 1,
						cell.as_str()
					);
					Paragraph::new(label)
						.block(block)
						.alignment(Alignment::Left)
						.style(
							Style::default()
								.fg(Color::Black)
								.bg(Color::Green)
						)
						.wrap(Wrap { trim: true })
				}
				else
				{
					Paragraph::new(cell.as_str())
						.block(block)
						.alignment(Alignment::Left)
						.style(Style::default())
						.wrap(Wrap { trim: true })
				};
			cell
		});
		// Render the solution. Colorize the quartiles. Highlight the last word,
		// which corresponds to the argument fragment path.
		self.render_solution_list(
			outer[1],
			buf,
			Some(solver),
			None,
			None::<&str>,
			Some(Style::default().fg(Color::White)),
			Some(Style::default()
				.fg(Color::Black)
				.bg(Color::Green)
			)
		);
	}

	/// Render the [finished](ExecutionState::Finished) UI.
	///
	/// # Arguments
	///
	/// * `area` - The target area.
	/// * `buf` - The target buffer.
	/// * `solver` - The solver.
	/// * `is_solved` - Whether the puzzle has been solved.
	/// * `highlight` - The index of the solution to highlight, if any.
	fn render_finished(
		&self,
		area: Rect,
		buf: &mut Buffer,
		solver: &Solver,
		is_solved: bool,
		highlight: Option<usize>
	) {
		// Split the screen into two parts: the puzzle and the solution.
		let outer = self.split_outer_screen(area);
		// The puzzle comprises a 4×5 grid of cells.
		let board = self.split_board(outer[0]);
		self.render_board(
			outer[0],
			buf,
			Some(
				if is_solved { "✓ Solved".green().bold() }
				else { "✗ No solution".red().bold() }
			),
			None::<&str>
		);
		// Render all of the cells.
		self.render_cells(board, buf, |_, cell| {
			let block = Block::new()
				.border_type(BorderType::Rounded)
				.borders(Borders::ALL)
				.border_style(Style::default().fg(Color::White));
			let cell = Paragraph::new(cell.as_str())
				.block(block)
				.alignment(Alignment::Left)
				.style(Style::default())
				.wrap(Wrap { trim: true });
			cell
		});
		// Render the solution. Colorize the quartiles. Highlight the selected
		// word.
		self.render_solution_list(
			outer[1],
			buf,
			Some(solver),
			Some(highlight),
			Some("↑↓ - move".cyan()),
			Some(Style::default().fg(Color::White)),
			Some(
				Style::default()
				.fg(Color::Black)
				.bg(Color::Cyan)
			)
		);
	}

	/// Split the specified area into two parts: the puzzle and the solution.
	///
	/// # Arguments
	///
	/// * `area` - The target area to split. This will be the complete screen
	///   available to the application.
	///
	/// # Returns
	///
	/// The split areas.
	fn split_outer_screen(&self, area: Rect) -> Rc<[Rect]>
	{
		Layout::default()
			.direction(Direction::Horizontal)
			.margin(1)
			.constraints([
				Constraint::Percentage(100),
				Constraint::Min(20)
			])
			.split(area)
	}

	/// Split the specified area into rows: two margins and 5 central
	/// rows.
	///
	/// # Arguments
	///
	/// * `area` - The target area to split.
	///
	/// # Returns
	///
	/// The split areas.
	fn split_board(&self, area: Rect) -> Rc<[Rect]>
	{
		Layout::default()
			.direction(Direction::Vertical)
			.margin(3)
			.constraints([
				Constraint::Ratio(1, 3),
				Constraint::Length(3),
				Constraint::Length(3),
				Constraint::Length(3),
				Constraint::Length(3),
				Constraint::Length(3),
				Constraint::Ratio(1, 3)
			])
			.split(area)
	}

	/// Render the board, with optional titles at the bottom center and top
	/// right.
	///
	/// # Arguments
	///
	/// * `area` - The target area.
	/// * `buf` - The target buffer.
	/// * `bottom_center` - The title to render at the bottom center.
	/// * `top_right` - The title to render at the top right.
	fn render_board<'a>(
		&self,
		area: Rect,
		buf: &mut Buffer,
		bottom_center: Option<impl Into<Line<'a>>>,
		top_right: Option<impl Into<Line<'a>>>
	) {
		let mut block = Block::default()
			.borders(Borders::ALL)
			.border_style(Style::default().fg(Color::White))
			.title(
				Title::default()
					.content("Puzzle")
					.position(Position::Top)
					.alignment(Alignment::Center)
			)
			.title(
				Title::default()
					.content("⎋ – exit".yellow().bold())
					.position(Position::Top)
					.alignment(Alignment::Left)
			);
		if let Some(title) = bottom_center
		{
			block = block.title(
				Title::default()
					.content(title)
					.position(Position::Bottom)
					.alignment(Alignment::Center)
			);
		}
		if let Some(title) = top_right
		{
			block = block.title(
				Title::default()
					.content(title)
					.position(Position::Top)
					.alignment(Alignment::Right)
			);
		}
		block.render(area, buf);
	}

	/// Render the cells of the board.
	///
	/// # Arguments
	///
	/// * `board` - The board area, as a margin, followed by 5 rows, followed by
	///   another margin.
	/// * `buf` - The target buffer.
	/// * `cell_builder` - A function that builds a cell from an index and a
	///   string.
	fn render_cells(
		&self,
		board: Rc<[Rect]>,
		buf: &mut Buffer,
		cell_builder: impl Fn(usize, &str8) -> Paragraph<'_>
	) {
		let cells = self.cells.iter().enumerate()
			.map(|(index, cell)| cell_builder(index, cell))
			.collect::<Vec<_>>();
		// Lay out the cells in a 4×5 grid.
		cells.chunks_exact(4).enumerate()
			.for_each(|(index, chunk)| {
				let row = Layout::default()
					.direction(Direction::Horizontal)
					.constraints([
						Constraint::Min(10),
						Constraint::Min(10),
						Constraint::Min(10),
						Constraint::Min(10)
					])
					.split(board[index + 1]);
				for (column, cell) in chunk.iter().enumerate()
				{
					cell.render(row[column], buf);
				}
			});
	}

	/// Construct a solution list from the solver, providing colorization based
	/// on the status of individual words. Specifically, quartiles are colored
	/// green, while shorter words are colored white. Deduplicate the list.
	///
	/// # Arguments
	///
	/// * `solver` - The solver.
	///
	/// # Returns
	///
	/// A list of styled text items.
	fn solution_list(&self, solver: &Solver) -> Vec<Text>
	{
		let mut seen = HashSet::new();
		solver.solution_paths().iter()
			.filter_map(|path| {
				let color = match path.is_full()
				{
					false => Color::White,
					true => Color::Green
				};
				let word = solver.word(path).to_string();
				let style = Style::default().fg(color);
				if seen.contains(&word)
				{
					None
				}
				else
				{
					seen.insert(word.clone());
					Some(Text::styled(word, style))
				}
			})
			.collect()
	}

	/// Render the solution list.
	///
	/// # Arguments
	///
	/// * `area` - The target area.
	/// * `buf` - The target buffer.
	/// * `solver` - The solver, which is only used in some application states.
	/// * `highlight` - The optional index of the highlighted item. If `None`,
	///   use the last item. If the inner `Option` is `None`, do not highlight
	///   any item.
	/// * `bottom_center` - The optional title to render at the bottom center.
	/// * `style` - The optional base style to apply to the list.
	/// * `highlight_style` - The optional style to apply to the highlighted
	///   item.
	#[allow(clippy::too_many_arguments)]
	fn render_solution_list<'a>(
		&self,
		area: Rect,
		buf: &mut Buffer,
		solver: Option<&Solver>,
		highlight: Option<Option<usize>>,
		bottom_center: Option<impl Into<Line<'a>>>,
		style: Option<Style>,
		highlight_style: Option<Style>
	) {
		let list = match solver
		{
			None => List::default(),
			Some(solver) => List::new(self.solution_list(solver))
		};
		let list = list
			.block({
				let block = Block::default()
					.borders(Borders::ALL)
					.title(
						Title::default()
							.content("Solution")
							.alignment(Alignment::Center)
					);
				match bottom_center
				{
					None => block,
					Some(title) => block.title(
						Title::default()
							.content(title)
							.position(Position::Bottom)
							.alignment(Alignment::Center)
					)
				}
			});
		let list = match style
		{
			None => list,
			Some(style) => list.style(style)
		};
		let list = match highlight_style
		{
			None => list,
			Some(highlight_style) => list.highlight_style(highlight_style)
		};
		let mut list_state = ListState::default();
		if let Some(solver) = solver
		{
			if let Some(highlight) = highlight
			{
				list_state.select(highlight);
			}
			else
			{
				list_state.select(Some(solver.solution().len() - 1));
			}
		}
		StatefulWidget::render(&list, area, buf, &mut list_state);
	}

	/// Run any background tasks, such as the solver or the highlighter.
	fn process_systems(&mut self)
	{
		match self.state
		{
			ExecutionState::Swapping => unreachable!(),
			ExecutionState::Populating => {}
			ExecutionState::Solving { .. } => self.run_solver(),
			ExecutionState::Highlighting { .. } => self.run_highlighter(),
			ExecutionState::Finished { .. } => {}
			ExecutionState::Exiting { .. } => {}
		}
	}

	/// Run the solver for a short while.
	fn run_solver(&mut self)
	{
		// Take care to evacuate the application state in order to keep the
		// borrow happy while juggling state ownership and mutable references.
		let mut state = ExecutionState::Swapping;
		swap(&mut self.state, &mut state);
		if let ExecutionState::Solving { solver } = state
		{
			// Run the solver for only a short while, lest the application
			// become unresponsive.
			let (solver, path) = solver.solve(Duration::from_millis(5));
			if solver.is_finished()
			{
				// The solver has finished.
				let is_solved = solver.is_solved();
				self.state = ExecutionState::Finished {
					solver,
					is_solved,
					highlight: None
				};
			}
			else if let Some(path) = path
			{
				// Highlight the most recently discovered solution.
				let until = Instant::now()
					+ Duration::from_millis(self.highlight_duration_µs);
				self.state = ExecutionState::Highlighting {
					solver,
					until,
					path
				};
			}
			else
			{
				// Maintain the solving state.
				self.state = ExecutionState::Solving { solver };
			}
		}
		else
		{
			unreachable!()
		}
	}

	/// Run the highlighter for a short while.
	fn run_highlighter(&mut self)
	{
		// Take care to evacuate the application state in order to keep the
		// borrow checker happy while juggling state ownership and mutable
		// references.
		let mut state = ExecutionState::Swapping;
		swap(&mut self.state, &mut state);
		if let ExecutionState::Highlighting { solver, until, path } = state
		{
			if Instant::now() >= until
			{
				// Return to the solving state.
				self.state = ExecutionState::Solving { solver };
			}
			else
			{
				// Maintain the highlighting.
				self.state = ExecutionState::Highlighting {
					solver,
					until,
					path
				};
			}
		}
		else
		{
			unreachable!()
		}
	}

	/// Process events. Block for only half a millisecond, so as not to stall
	/// any background tasks.
	///
	/// # Errors
	///
	/// Any error that occurs while processing events.
	fn process_event(&mut self) -> io::Result<()>
	{
		if poll(Duration::from_micros(500))?
		{
			match read()?
			{
				Event::Key(event) if event.kind == KeyEventKind::Press =>
					self.process_key_event(event),
				_ => {}
			}
		}
		Ok(())
	}

	/// Process a key event:
	///
	/// * Escape - Exit the application.
	/// * Up - Move the cursor up.
	/// * Down - Move the cursor down.
	/// * Left - Move the cursor left.
	/// * Right - Move the cursor right.
	/// * BackTab - (Shift+Tab) Move the cursor to the previous cell.
	/// * Tab - Move the cursor to the next cell.
	/// * Backspace - Delete the last character of the current cell.
	/// * A-Z - Append the corresponding character to the current cell.
	///
	/// # Arguments
	///
	/// * `event` - The key event to process.
	fn process_key_event(&mut self, event: KeyEvent)
	{
		match self.state
		{
			ExecutionState::Swapping => unreachable!(),
			ExecutionState::Populating =>
				self.process_key_event_populating(event),
			ExecutionState::Solving { .. } =>
				self.process_key_event_solving(event),
			ExecutionState::Highlighting { .. } =>
				self.process_key_event_highlighting(event),
			ExecutionState::Finished { .. } =>
				self.process_key_event_finished(event),
			ExecutionState::Exiting { .. } => {}
		}
	}

	/// Process a key event while [populating](ExecutionState::Populating) the
	/// puzzle:
	///
	/// * Escape - Exit the application.
	/// * Up - Move the cursor up.
	/// * Down - Move the cursor down.
	/// * Left - Move the cursor left.
	/// * Right - Move the cursor right.
	/// * BackTab - (Shift+Tab) Move the cursor to the previous cell.
	/// * Tab - Move the cursor to the next cell.
	/// * Backspace - Delete the last character of the current cell.
	/// * Enter - Solve the puzzle.
	/// * A-Z - Append the corresponding character to the current cell.
	///
	/// # Arguments
	///
	/// * `event` - The key event to process.
	fn process_key_event_populating(&mut self, event: KeyEvent)
	{
		match event.code
		{
			KeyCode::Esc => self.exit(),
			KeyCode::Up => self.move_cursor(0, -1),
			KeyCode::Down => self.move_cursor(0, 1),
			KeyCode::Left => self.move_cursor(-1, 0),
			KeyCode::Right => self.move_cursor(1, 0),
			KeyCode::BackTab => self.move_index(-1),
			KeyCode::Tab => self.move_index(1),
			KeyCode::Backspace => self.delete(),
			KeyCode::Delete if event.modifiers.contains(KeyModifiers::SHIFT) =>
				self.clear_all(),
			KeyCode::Delete => self.clear(),
			KeyCode::Enter => self.start_solver(),
			KeyCode::Char(c) if c.is_alphabetic() => self.append(c),
			_ => {}
		}
	}

	/// Attempt to start the solver. If the puzzle is not fully populated, do
	/// nothing; the UI already provides feedback to the user.
	fn start_solver(&mut self)
	{
		if self.cells.iter().all(|cell| !cell.is_empty())
		{
			let solver = Solver::new(self.dictionary.clone(), self.cells);
			self.state = ExecutionState::Solving { solver };
		}
	}

	/// Process a key event while [solving](ExecutionState::Solving) the
	/// puzzle:
	///
	/// * Escape - Exit the application.
	///
	/// Also, run the solver for a short while, potentially highlighting the
	/// most recently discovered solution.
	///
	/// # Arguments
	///
	/// * `event` - The key event to process.
	/// * `solver` - The solver.
	fn process_key_event_solving(&mut self, event: KeyEvent)
	{
		if let KeyCode::Esc = event.code {
			self.exit()
		}
	}

	/// Process a key event while [highlighting](ExecutionState::Highlighting)
	/// the puzzle:
	///
	/// * Escape - Exit the application.
	///
	/// Maintain the highlight for long enough to be visible, then return to the
	/// [solving](ExecutionState::Solving) state.
	///
	/// # Arguments
	///
	/// * `event` - The key event to process.
	/// * `solver` - The solver.
	fn process_key_event_highlighting(&mut self, event: KeyEvent)
	{
		if let KeyCode::Esc = event.code {
			self.exit()
		}
	}

	/// Process a key event while [reviewing](ExecutionState::Finished) the
	/// solution:
	///
	/// * Escape - Exit the application.
	///
	/// # Arguments
	///
	/// * `event` - The key event to process.
	/// * `solver` - The solver.
	fn process_key_event_finished(&mut self, event: KeyEvent)
	{
		match event.code
		{
			KeyCode::Esc => self.exit(),
			KeyCode::Up => self.move_word_index(-1),
			KeyCode::Down => self.move_word_index(1),
			_ => {}
		}
	}

	/// Mark the application for exit. The application will exit after the next
	/// iteration of the main loop.
	fn exit(&mut self)
	{
		let next_state = match self.state
		{
			ExecutionState::Swapping => unreachable!(),
			ExecutionState::Populating =>
				ExecutionState::Exiting { solution: vec![] },
			ExecutionState::Solving { .. } =>
				ExecutionState::Exiting { solution: vec![] },
			ExecutionState::Highlighting { .. } =>
				ExecutionState::Exiting { solution: vec![] },
			ExecutionState::Finished { ref solver, .. } =>
				ExecutionState::Exiting {
					solution: solver.solution().iter()
						.map(|s| s.to_string()).collect()
				},
			ExecutionState::Exiting { ref solution } =>
				ExecutionState::Exiting { solution: solution.clone() }
		};
		self.state = next_state;
	}
}

impl Widget for &App
{
	fn render(self, area: Rect, buf: &mut Buffer)
	{
		match self.state
		{
			ExecutionState::Swapping => unreachable!(),
			ExecutionState::Populating => self.render_populating(area, buf),
			ExecutionState::Solving { ref solver } =>
				self.render_solving(area, buf, solver),
			ExecutionState::Highlighting { ref solver, ref path, .. } =>
				self.render_highlighting(area, buf, solver, path),
			ExecutionState::Finished { ref solver, is_solved, highlight } =>
				self.render_finished(area, buf, solver, is_solved, highlight),
			ExecutionState::Exiting { .. } => {}
		}
	}
}

/// The execution state of the application.
#[derive(Clone, Debug)]
enum ExecutionState
{
	/// The application state is transitioning to the next state. This is a
	/// transient state that should not be rendered.
	Swapping,

	/// The user is populating the puzzle with fragments.
	Populating,

	/// The solver is running, incrementally populating the solution.
	Solving {
		/// The solver for the puzzle.
		solver: Solver,
	},

	/// The solver is highlighting the most recently discovered solution, and
	/// will momentarily return to the [Solving](ExecutionState::Solving) state.
	Highlighting {
		/// The solver for the puzzle.
		solver: Solver,

		/// When to transition back to the [Solving](ExecutionState::Solving)
		/// state.
		until: Instant,

		/// The fragment path of the solution to highlight.
		path: FragmentPath
	},

	/// The solver has finished, but the user is reviewing the solution.
	Finished {
		/// The solver for the puzzle.
		solver: Solver,

		/// Whether a complete solution was found.
		is_solved: bool,

		/// The index of the word to highlight in the solution.
		highlight: Option<usize>
	},

	/// The application is exiting.
	Exiting {
		/// The solver for the puzzle.
		solution: Vec<String>
	}
}

////////////////////////////////////////////////////////////////////////////////
//                                   Tests.                                   //
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test
{
	use super::*;

	/// Ensure that the application exits when the escape key is pressed.
	#[test]
	fn test_handle_exit()
	{
		let mut app = App::new(0, Dictionary::default());
		assert!(app.is_running());
		app.process_key_event(KeyCode::Esc.into());
		assert!(!app.is_running());
	}

	/// Ensure that the cursor moves up, down, left, and right when the
	/// corresponding arrow keys are pressed. Test all possible cursor
	/// movements.
	#[test]
	fn test_handle_arrows()
	{
		let mut app = App::new(0, Dictionary::default());
		assert_eq!(app.cursor, (0, 0));
		// Test all possible cursor movements. Each case is a tuple of the
		// initial cursor position and the expected cursor position after
		// moving up, right, down, and left, respectively.
		let cases = vec![
			((0, 0), [(0, 0), (1, 0), (0, 1), (0, 0)]),
			((0, 1), [(0, 0), (1, 1), (0, 2), (0, 1)]),
			((0, 2), [(0, 1), (1, 2), (0, 3), (0, 2)]),
			((0, 3), [(0, 2), (1, 3), (0, 4), (0, 3)]),
			((0, 4), [(0, 3), (1, 4), (0, 4), (0, 4)]),
			((1, 0), [(1, 0), (2, 0), (1, 1), (0, 0)]),
			((1, 1), [(1, 0), (2, 1), (1, 2), (0, 1)]),
			((1, 2), [(1, 1), (2, 2), (1, 3), (0, 2)]),
			((1, 3), [(1, 2), (2, 3), (1, 4), (0, 3)]),
			((1, 4), [(1, 3), (2, 4), (1, 4), (0, 4)]),
			((2, 0), [(2, 0), (3, 0), (2, 1), (1, 0)]),
			((2, 1), [(2, 0), (3, 1), (2, 2), (1, 1)]),
			((2, 2), [(2, 1), (3, 2), (2, 3), (1, 2)]),
			((2, 3), [(2, 2), (3, 3), (2, 4), (1, 3)]),
			((2, 4), [(2, 3), (3, 4), (2, 4), (1, 4)]),
			((3, 0), [(3, 0), (3, 0), (3, 1), (2, 0)]),
			((3, 1), [(3, 0), (3, 1), (3, 2), (2, 1)]),
			((3, 2), [(3, 1), (3, 2), (3, 3), (2, 2)]),
			((3, 3), [(3, 2), (3, 3), (3, 4), (2, 3)]),
			((3, 4), [(3, 3), (3, 4), (3, 4), (2, 4)])
		];
		for (initial, expected) in cases
		{
			app.cursor = initial;
			app.process_key_event(KeyCode::Up.into());
			assert_eq!(app.cursor, expected[0], "up");
			app.cursor = initial;
			app.process_key_event(KeyCode::Right.into());
			assert_eq!(app.cursor, expected[1], "right");
			app.cursor = initial;
			app.process_key_event(KeyCode::Down.into());
			assert_eq!(app.cursor, expected[2], "down");
			app.cursor = initial;
			app.process_key_event(KeyCode::Left.into());
			assert_eq!(app.cursor, expected[3], "left");
		}
	}

	/// Ensure that the cursor moves to the next cell when the tab key is
	/// pressed.
	#[test]
	fn test_handle_tab()
	{
		let mut app = App::new(0, Dictionary::default());
		assert_eq!(app.cursor, (0, 0));
		// Test all possible cursor movements. Each case is a tuple of the
		// initial cursor position and the expected cursor position after
		// tab and shift-tab, respectively.
		let cases = vec![
			((0, 0), [(1, 0), (0, 0)]),
			((1, 0), [(2, 0), (0, 0)]),
			((2, 0), [(3, 0), (1, 0)]),
			((3, 0), [(0, 1), (2, 0)]),
			((0, 1), [(1, 1), (3, 0)]),
			((1, 1), [(2, 1), (0, 1)]),
			((2, 1), [(3, 1), (1, 1)]),
			((3, 1), [(0, 2), (2, 1)]),
			((0, 2), [(1, 2), (3, 1)]),
			((1, 2), [(2, 2), (0, 2)]),
			((2, 2), [(3, 2), (1, 2)]),
			((3, 2), [(0, 3), (2, 2)]),
			((0, 3), [(1, 3), (3, 2)]),
			((1, 3), [(2, 3), (0, 3)]),
			((2, 3), [(3, 3), (1, 3)]),
			((3, 3), [(0, 4), (2, 3)]),
			((0, 4), [(1, 4), (3, 3)]),
			((1, 4), [(2, 4), (0, 4)]),
			((2, 4), [(3, 4), (1, 4)]),
			((3, 4), [(3, 4), (2, 4)])
		];
		for (initial, expected) in cases
		{
			app.cursor = initial;
			app.process_key_event(KeyCode::Tab.into());
			assert_eq!(app.cursor, expected[0], "tab");
			app.cursor = initial;
			app.process_key_event(KeyCode::BackTab.into());
			assert_eq!(app.cursor, expected[1], "shift-tab");
		}
	}

	/// Ensure that the current cell is edited correctly when alphabetic
	/// characters are appended and deleted.
	#[test]
	fn test_handle_edit()
	{
		let mut app = App::new(0, Dictionary::default());
		assert_eq!(app.current_cell(), &str8::default());
		// Test deleting from an empty cell.
		app.process_key_event(KeyCode::Backspace.into());
		assert_eq!(app.current_cell(), &str8::default());
		// Test appending and deleting all alphabetic characters.
		for c in 'a' ..= 'z'
		{
			app.process_key_event(KeyCode::Char(c).into());
			assert_eq!(app.current_cell(), &str8::make(&c.to_string()));
			app.process_key_event(KeyCode::Backspace.into());
			assert_eq!(app.current_cell(), &str8::default());
		}
		// Test saturating the cell.
		let mut s = String::new();
		for c in 'a' ..= 'j'
		{
			s.push(c);
			app.process_key_event(KeyCode::Char(c).into());
			// Take the first 7 characters from the string.
			let s = s.chars().take(7).collect::<String>();
			assert_eq!(app.current_cell(), &str8::make(&s));
		}
	}
}
