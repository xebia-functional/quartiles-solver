//! # Solver
//!
//! Herein is the solver for the Quartiles game.

use std::{
	collections::HashSet,
	error::Error,
	fmt::{self, Display, Formatter},
	ops::{Index, IndexMut},
	rc::Rc,
	time::{Duration, Instant}
};

use fixedstr::{str32, str8};
use log::{debug, trace};

use crate::dictionary::Dictionary;

////////////////////////////////////////////////////////////////////////////////
//                                  Solver.                                   //
////////////////////////////////////////////////////////////////////////////////

/// The complete context of the Quartiles solver. This permits an iterative
/// solution to the puzzle, rather than a recursive one. An iterative solution
/// can be time-sliced and parallelized.
#[derive(Clone, Debug)]
#[must_use]
pub struct Solver
{
	/// The dictionary to use for solving the puzzle.
	dictionary: Rc<Dictionary>,

	/// The fragments of the puzzle.
	fragments: [str8; 20],

	/// The current fragment path.
	path: FragmentPath,

	/// The solution to the puzzle, as a list of fragment paths.
	solution: Vec<FragmentPath>,

	/// Whether the solver is finished.
	is_finished: bool
}

impl Solver
{
	/// Construct a new solver for the given dictionary.
	///
	/// # Arguments
	///
	/// * `dictionary` - The dictionary to use for solving the puzzle.
	/// * `fragments` - The fragments of the puzzle.
	///
	/// # Returns
	///
	/// A new solver for the given dictionary.
	pub fn new(dictionary: Rc<Dictionary>, fragments: [str8; 20]) -> Self
	{
		Self
		{
			dictionary,
			fragments,
			path: Default::default(),
			solution: Vec::new(),
			is_finished: false
		}
	}

	/// Check if the solver is finished. The solver is finished if the search
	/// algorithm has terminated due to exhaustion of the search space.
	///
	/// # Returns
	///
	/// `true` if the solver is finished, `false` otherwise.
	#[inline]
	#[must_use]
	pub fn is_finished(&self) -> bool
	{
		self.is_finished
	}

	/// Check if the solver has produced a complete solution. This requires not
	/// only that the solver [finished](Self::is_finished), but also that 5 full
	/// fragment paths have been found, and that every fragment has been used.
	/// If the user has misentered the puzzle or supplied an unofficial puzzle,
	/// the solver may finish without producing a complete solution.
	///
	/// # Returns
	///
	/// `true` if the solver has produced a complete solution, `false`
	/// otherwise.
	pub fn is_solved(&self) -> bool
	{
		if !self.is_finished
		{
			// The solver hasn't even finished running, so there's no point
			// checking whether the solution is complete. It technically
			// might be, but it would be jumping the gun to say so.
			return false
		}
		let full_paths = self.solution.iter()
			.filter(|p| p.is_full())
			.collect::<Vec<_>>();
		let unique = full_paths.iter()
			.map(|p| p.word(&self.fragments).to_string())
			.collect::<HashSet<_>>();
		// We expect exactly 5 full fragment paths in the solution to an
		// official Quartiles puzzle. We allow for more, in case someone has
		// supplied an unofficial puzzle.
		if unique.len() < 5
		{
			return false
		}
		// We have only obtained a solution if every fragment has been used.
		// For an official puzzle, this should occur automatically when 5
		// full fragment paths are found, but may not be the case for an
		// unofficial puzzle.
		let used_indices = full_paths.iter()
			.flat_map(|p| p.0.iter().flatten())
			.collect::<HashSet<_>>();
		used_indices.len() == self.fragments.len()
	}

	/// Run the solver until a single valid word is found or the specified
	/// quantum elapses. Always process at least one fragment path, even if
	/// the quantum is zero, to ensure that the solver always makes progress.
	///
	/// # Arguments
	///
	/// * `duration` - The maximum amount of time to run the solver before
	///   answering a continuation context.
	///
	/// # Returns
	///
	/// A 2-tuple comprising the continuation context and any valid word found,
	/// respectively. The caller should call [`is_finished`](Self::is_finished)
	/// to determine if there is any additional work to perform.
	pub fn solve(mut self, duration: Duration) -> (Self, Option<FragmentPath>)
	{
		// Ensure that the current fragment path is prima facie valid.
		assert!(self.path.is_disjoint());

		// If the solver is already finished, just return it.
		if self.is_finished
		{
			trace!("solver is already finished");
			return (self, None)
		}

		// Start the timer. Loop until the timer expires or a single valid word
		// is discovered.
		let start_time = Instant::now();
		let mut found_word = false;
		loop
		{
			let start_path = self.path;
			trace!("considering: {}", self.current_word());

			// If the current fragment path corresponds to a valid word, then
			// add it to the solution. Note that we discovered a valid word, so
			// that we can return control to the caller after deriving the next
			// context.
			if self.dictionary.contains(self.current_word().as_str())
			{
				debug!("found word: {}", self.current_word());
				self.solution.push(self.path);
				found_word = true;
			}

			// If the current fragment path does not denote the prefix of any
			// word in the dictionary, then there is no need to continue
			// searching along this path.
			if self.dictionary.contains_prefix(self.current_word().as_str())
			{
				// Try to append the next fragment index.
				match self.path.append()
				{
					Ok(path) =>
					{
						// The next fragment index was successfully appended, so
						// continue the search.
						trace!(
							"next after append: {:?} => {}",
							path,
							path.word(&self.fragments)
						);
						self.path = path;
					}
					Err(FragmentPathError::Overflow) =>
					{
						// The fragment path is already full, so there's nothing
						// to do here. Just continue the algorithm.
					}
					Err(_) => unreachable!()
				}
			}

			if self.path == start_path
			{
				// We didn't append a new fragment index, so try to increment
				// the rightmost fragment index instead.
				match self.path.increment()
				{
					Ok(path) =>
					{
						// The rightmost fragment index was successfully
						// incremented, so continue the search.
						trace!(
							"next after increment: {:?} => {}",
							path,
							path.word(&self.fragments)
						);
						self.path = path;
					}
					Err(FragmentPathError::IndexOverflow) =>
					{
						// The rightmost fragment index is already at the
						// maximum, so try to pop it and increment the previous
						// fragment index.
						match self.path.pop_and_increment()
						{
							Ok(path) =>
							{
								// The rightmost fragment index was popped and
								// the previous fragment index incremented, so
								// continue the search.
								trace!(
									"next after pop and increment: {:?} => {}",
									path,
									self.current_word()
								);
								self.path = path;
							}
							// The fragment path is now empty, so we have
							// exhausted the search space.
							Err(FragmentPathError::CannotIncrementEmpty) =>
							{
								debug!("exhausted search space");
								self.is_finished = true;
								return (self, None)
							}
							Err(_) => unreachable!()
						}
					}
					Err(_) => unreachable!()
				}
			}

			// Ensure that the solver is making progress.
			assert_ne!(
				self.path,
				start_path,
				"solver failed to make progress: {:?} => {}",
				self.path,
				self.current_word()
			);

			if found_word
			{
				// The solver has found a valid word, so return the next
				// context.
				let word = *self.solution.last().unwrap();
				return (self, Some(word))
			}

			let elapsed = Instant::now().duration_since(start_time);
			if elapsed >= duration
			{
				// The solver has run out of time, so return the current
				// context.
				trace!("quantum elapsed: {:?}", elapsed);
				return (self, None)
			}
		}
	}

	/// Run the solver until the search space is exhausted.
	///
	/// # Returns
	///
	/// The final context, which must contain a complete solution if the puzzle
	/// is solvable.
	pub fn solve_fully(mut self) -> Self
	{
		while !self.is_finished
		{
			let next = self.solve(Duration::from_secs(u64::MAX));
			self = next.0;
		}
		self
	}

	/// Get the candidate word corresponding to the specified fragment path.
	///
	/// # Arguments
	///
	/// * `path` - The fragment path.
	///
	/// # Returns
	///
	/// The candidate word corresponding to the specified fragment path.
	#[inline]
	#[must_use]
	pub fn word(&self, path: &FragmentPath) -> str32
	{
		path.word(&self.fragments)
	}

	/// Get the candidate word corresponding to the current fragment path.
	///
	/// # Returns
	///
	/// The candidate word corresponding to the current fragment path.
	#[inline]
	#[must_use]
	fn current_word(&self) -> str32
	{
		self.path.word(&self.fragments)
	}

	/// Get the solution to the puzzle, as a list of fragment paths.
	///
	/// # Returns
	///
	/// The solution to the puzzle, as a list of fragment paths.
	#[inline]
	#[must_use]
	pub fn solution_paths(&self) -> Vec<FragmentPath>
	{
		self.solution.clone()
	}

	/// Get the solution to the puzzle, as a list of words.
	///
	/// # Returns
	///
	/// The solution to the puzzle, as a list of words.
	#[inline]
	#[must_use]
	pub fn solution(&self) -> Vec<str32>
	{
		self.solution.iter()
			.map(|p| p.word(&self.fragments))
			.collect()
	}
}

////////////////////////////////////////////////////////////////////////////////
//                              Fragment paths.                               //
////////////////////////////////////////////////////////////////////////////////

/// A fragment path is a sequence of four or fewer fragment indices that
/// correspond to a candidate word. The fragment path is filled in order,
/// from left to right, and vacated in reverse order, from right to left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct FragmentPath([Option<usize>; 4]);

impl FragmentPath
{
	/// Get an iterator over the fragment indices in the fragment path. The
	/// iterator yields `None` for any unused fragment indices.
	///
	/// # Returns
	///
	/// An iterator over the fragment indices in the fragment path.
	#[inline]
	pub fn iter(&self) -> impl Iterator<Item = Option<usize>> + '_
	{
		self.0.iter().copied()
	}

	/// Check if the fragment path is empty.
	///
	/// # Returns
	///
	/// `true` if the fragment path is empty, `false` otherwise.
	#[inline]
	#[must_use]
	pub fn is_empty(&self) -> bool
	{
		self.0[0].is_none()
	}

	/// Check if the fragment path is full.
	///
	/// # Returns
	///
	/// `true` if the fragment path is full, `false` otherwise.
	#[inline]
	#[must_use]
	pub fn is_full(&self) -> bool
	{
		self.0[3].is_some()
	}

	/// Append a fragment index to the fragment path, using the existing
	/// fragment indices as uniqueness constraints. The result is always a
	/// [valid](Self::is_disjoint) fragment path.
	///
	/// # Returns
	///
	/// The fragment path with the fragment index appended.
	///
	/// # Errors
	///
	/// [`FragmentPathError::Overflow`] if the fragment path is already full.
	fn append(&self) -> Result<Self, FragmentPathError>
	{
		if self.is_full()
		{
			Err(FragmentPathError::Overflow)
		}
		else
		{
			// Find the index of the rightmost occupant.
			let rightmost = self.0.iter()
				.rposition(|&index| index.is_some())
				.map(|i| i as i32)
				.unwrap_or(-1);
			// Determine which fragment indices are unavailable.
			let used = HashSet::<usize>::from_iter(
				self.0.iter().flatten().copied()
			);
			// Determine the start index for the new fragment index.
			let mut start_index = 0;
			while used.contains(&start_index)
			{
				start_index += 1;
			}
			// Append the next fragment index.
			let mut fragment = *self;
			fragment[(rightmost + 1) as usize] = Some(start_index);
			Ok(fragment)
		}
	}

	/// Increment the rightmost fragment index in the fragment path, using the
	/// other fragment indices as uniqueness constraints. The result is always
	/// a [valid](Self::is_disjoint) fragment path.
	///
	/// # Returns
	///
	/// The fragment path with the rightmost fragment index incremented.
	///
	/// # Errors
	///
	/// * [`FragmentPathError::CannotIncrementEmpty`] if the fragment path is
	///   empty.
	/// * [`FragmentPathError::IndexOverflow`] if the rightmost fragment index
	///   is already at the maximum value.
	fn increment(&self) -> Result<Self, FragmentPathError>
	{
		// Find the index of the rightmost occupant.
		let rightmost = self.0.iter()
			.rposition(|&index| index.is_some())
			.ok_or(FragmentPathError::CannotIncrementEmpty)?;
		// Determine which fragment indices are unavailable. Use all but the
		// last fragment index, because the last fragment index is the one that
		// is incremented.
		let used = HashSet::<usize>::from_iter(
			self.0.iter().take(rightmost).flatten().copied()
		);
		// Determine the stop index for the rightmost fragment index.
		let mut stop_index = 19;
		while used.contains(&stop_index)
		{
			stop_index -= 1;
		}
		let mut fragment = *self;
		loop
		{
			if fragment[rightmost] >= Some(stop_index)
			{
				// The rightmost fragment index is already at (or beyond) the
				// maximum value, so report an overflow.
				return Err(FragmentPathError::IndexOverflow)
			}
			else
			{
				// Increment the rightmost fragment index.
				let next = fragment[rightmost].unwrap() + 1;
				fragment[rightmost] = Some(next);
				if !used.contains(&next)
				{
					// The incremented fragment index is available, so use it.
					return Ok(fragment)
				}
			}
		}
	}

	/// Pop a fragment index from the fragment path.
	///
	/// # Returns
	///
	/// The fragment path with the last fragment index popped.
	///
	/// # Errors
	///
	/// [`FragmentPathError::Underflow`] if the fragment path is already empty.
	fn pop(&self) -> Result<Self, FragmentPathError>
	{
		if self.is_empty()
		{
			Err(FragmentPathError::Underflow)
		}
		else
		{
			let mut indices = self.0;
			let rightmost = indices.iter()
				.rposition(|&index| index.is_some())
				.unwrap();
			indices[rightmost] = None;
			Ok(Self(indices))
		}
	}

	/// Iteratively pop the rightmost fragment index and increment the previous
	/// fragment until a valid fragment path is obtained.
	///
	/// # Returns
	///
	/// The next valid fragment path in the sequence.
	///
	/// # Errors
	///
	/// * [`FragmentPathError::Underflow`] if the fragment path is already
	///   empty.
	/// * [`FragmentPathError::CannotIncrementEmpty`] if the fragment path is
	///   empty after popping.
	fn pop_and_increment(&self) -> Result<Self, FragmentPathError>
	{
		let mut fragment = *self;
		loop
		{
			fragment = fragment.pop()?;
			match fragment.increment()
			{
				Ok(fragment) => return Ok(fragment),
				Err(FragmentPathError::IndexOverflow) => continue,
				Err(FragmentPathError::CannotIncrementEmpty) =>
					return Err(FragmentPathError::CannotIncrementEmpty),
				Err(_) => unreachable!()
			}
		}
	}

	/// Check if the fragment indices are disjoint. All valid fragment paths are
	/// disjoint.
	///
	/// # Returns
	///
	/// `true` if the fragment indices are disjoint, `false` otherwise.
	fn is_disjoint(&self) -> bool
	{
		let mut seen = [false; 20];
		for &index in self.0.iter().flatten()
		{
			if seen[index]
			{
				return false
			}
			seen[index] = true
		}
		true
	}

	/// Get the candidate word corresponding to the fragment path.
	///
	/// # Arguments
	///
	/// * `fragments - The fragments of the puzzle.
	///
	/// # Returns
	///
	/// The candidate word corresponding to the fragment path.
	#[inline]
	#[must_use]
	fn word(&self, fragments: &[str8; 20]) -> str32
	{
		let mut word = str32::new();
		for &index in self.0.iter().flatten()
		{
			word.push(&fragments[index]);
		}
		word
	}
}

impl Index<usize> for FragmentPath
{
	type Output = Option<usize>;

	#[inline]
	fn index(&self, index: usize) -> &Self::Output
	{
		&self.0[index]
	}
}

impl IndexMut<usize> for FragmentPath
{
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut Self::Output
	{
		&mut self.0[index]
	}
}

/// The complete enumeration of [`FragmentPath`] errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FragmentPathError
{
	/// The fragment path is already full, so no more fragments can be appended.
	Overflow,

	/// The fragment path is already empty, so no more fragments can be popped.
	Underflow,

	/// The fragment index is already at the maximum value of 19, so it cannot
	/// be incremented.
	IndexOverflow,

	/// The fragment path is empty, so it cannot be incremented.
	CannotIncrementEmpty
}

impl Display for FragmentPathError
{
	fn fmt(&self, f: &mut Formatter) -> fmt::Result
	{
		match self
		{
			Self::Overflow => write!(f, "fragment path is already full"),
			Self::Underflow => write!(f, "fragment path is already empty"),
			Self::IndexOverflow =>
				write!(f, "fragment index is already at maximum"),
			Self::CannotIncrementEmpty => write!(f, "fragment path is empty")
		}
	}
}

impl Error for FragmentPathError {}

////////////////////////////////////////////////////////////////////////////////
//                                   Tests.                                   //
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test
{
	use std::{collections::HashSet, rc::Rc};
	use crate::{
		dictionary::Dictionary,
		solver::{FragmentPath, FragmentPathError, Solver}
	};
	use fixedstr::{str32, str8};

	/// Ensure that appending a fragment index to a fragment path works for all
	/// interesting cases.
	#[test]
	fn test_append()
	{
		let path = FragmentPath::default();
		assert_eq!(path, FragmentPath([None, None, None, None]));
		assert!(path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(0), None, None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), Some(2), None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), Some(2), Some(3)]));
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(path.append(), Err(FragmentPathError::Overflow));
	}

	/// Ensure that popping a fragment index from a fragment path works for all
	/// interesting cases.
	#[test]
	fn test_increment()
	{
		let mut path = FragmentPath::default();
		assert_eq!(
			path.increment(),
			Err(FragmentPathError::CannotIncrementEmpty)
		);

		path = path.append().unwrap();
		for i in 0..19
		{
			assert_eq!(path, FragmentPath([Some(i), None, None, None]));
			assert!(!path.is_empty());
			assert!(!path.is_full());
			assert!(path.is_disjoint());
			path = path.increment().unwrap();
		}
		assert_eq!(path, FragmentPath([Some(19), None, None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(path.increment(), Err(FragmentPathError::IndexOverflow));

		path = path.append().unwrap();
		for i in 0..18
		{
			assert_eq!(path, FragmentPath([Some(19), Some(i), None, None]));
			assert!(!path.is_empty());
			assert!(!path.is_full());
			assert!(path.is_disjoint());
			path = path.increment().unwrap();
		}
		assert_eq!(path, FragmentPath([Some(19), Some(18), None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(path.increment(), Err(FragmentPathError::IndexOverflow));

		path = path.append().unwrap();
		for i in 0..17
		{
			assert_eq!(path, FragmentPath([Some(19), Some(18), Some(i), None]));
			assert!(!path.is_empty());
			assert!(!path.is_full());
			assert!(path.is_disjoint());
			path = path.increment().unwrap();
		}
		assert_eq!(path, FragmentPath([Some(19), Some(18), Some(17), None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(path.increment(), Err(FragmentPathError::IndexOverflow));

		path = path.append().unwrap();
		for i in 0..16
		{
			assert_eq!(
				path,
				FragmentPath([Some(19), Some(18), Some(17), Some(i)])
			);
			assert!(!path.is_empty());
			assert!(path.is_full());
			assert!(path.is_disjoint());
			path = path.increment().unwrap();
		}
		assert_eq!(
			path,
			FragmentPath([Some(19), Some(18), Some(17), Some(16)])
		);
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(path.increment(), Err(FragmentPathError::IndexOverflow));

		path = FragmentPath([Some(1), Some(2), Some(3), None]);
		path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(1), Some(2), Some(3), Some(0)]));
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(
			path.increment().unwrap(),
			FragmentPath([Some(1), Some(2), Some(3), Some(4)])
		);

		path = FragmentPath([Some(1), Some(19), Some(3), None]);
		path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(1), Some(19), Some(3), Some(0)]));
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		path = path.increment().unwrap();
		assert_eq!(
			path,
			FragmentPath([Some(1), Some(19), Some(3), Some(2)])
		);
		path = path.increment().unwrap();
		for i in 4..18
		{
			assert_eq!(
				path,
				FragmentPath([Some(1), Some(19), Some(3), Some(i)])
			);
			assert!(!path.is_empty());
			assert!(path.is_full());
			assert!(path.is_disjoint());
			path = path.increment().unwrap();
		}
		assert_eq!(
			path,
			FragmentPath([Some(1), Some(19), Some(3), Some(18)])
		);
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(path.increment(), Err(FragmentPathError::IndexOverflow));
	}

	/// Ensure that popping a fragment index from a fragment path works for all
	/// interesting cases.
	#[test]
	fn test_pop()
	{
		let path = FragmentPath::default();
		assert_eq!(
			path.pop(),
			Err(FragmentPathError::Underflow)
		);

		let path = path.append().unwrap();
		let path = path.append().unwrap();
		let path = path.append().unwrap();
		let path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), Some(2), Some(3)]));
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), Some(2), None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop().unwrap();
		assert_eq!(path, FragmentPath([Some(0), None, None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop().unwrap();
		assert_eq!(path, FragmentPath([None, None, None, None]));
		assert!(path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
	}

	/// Ensure that popping and incrementing a fragment path works for all
	/// interesting cases.
	#[test]
	fn test_pop_and_increment()
	{
		let path = FragmentPath::default();
		assert_eq!(
			path.pop_and_increment(),
			Err(FragmentPathError::Underflow)
		);

		let path = path.append().unwrap();
		let path = path.append().unwrap();
		let path = path.append().unwrap();
		let path = path.append().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), Some(2), Some(3)]));
		assert!(!path.is_empty());
		assert!(path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop_and_increment().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(1), Some(3), None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop_and_increment().unwrap();
		assert_eq!(path, FragmentPath([Some(0), Some(2), None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		let path = path.pop_and_increment().unwrap();
		assert_eq!(path, FragmentPath([Some(1), None, None, None]));
		assert!(!path.is_empty());
		assert!(!path.is_full());
		assert!(path.is_disjoint());
		assert_eq!(
			path.pop_and_increment(),
			Err(FragmentPathError::CannotIncrementEmpty)
		);

		let path = FragmentPath([Some(19), Some(18), Some(17), Some(16)]);
		assert_eq!(
			path.pop_and_increment(),
			Err(FragmentPathError::CannotIncrementEmpty)
		);

		let path = FragmentPath([Some(18), Some(17), Some(16), Some(15)]);
		let path = path.pop_and_increment().unwrap();
		assert_eq!(path, FragmentPath([Some(18), Some(17), Some(19), None]));
		let path = path.pop_and_increment().unwrap();
		assert_eq!(path, FragmentPath([Some(18), Some(19), None, None]));
		let path = path.pop_and_increment().unwrap();
		assert_eq!(path, FragmentPath([Some(19), None, None, None]));
		assert_eq!(
			path.pop_and_increment(),
			Err(FragmentPathError::CannotIncrementEmpty)
		);
	}

	/// Ensure that the disjointedness of fragment paths is correctly
	/// determined. Be exhaustive, since it's cheap and the space is easy to
	/// enumerate.
	#[test]
	fn test_is_disjoint()
	{
		let path = FragmentPath::default();
		assert!(path.is_disjoint());

		for i in 0..20
		{
			let path = FragmentPath([Some(i), None, None, None]);
			assert!(path.is_disjoint());
		}

		for i in 0..20
		{
			for j in 0..20
			{
				let path = FragmentPath([Some(i), Some(j), None, None]);
				assert_eq!(path.is_disjoint(), i != j, "{}, {}", i, j);
			}
		}

		for i in 0..20
		{
			for j in 0..20
			{
				for k in 0..20
				{
					let path = FragmentPath([Some(i), Some(j), Some(k), None]);
					assert_eq!(
						path.is_disjoint(),
						i != j && i != k && j != k,
						"{}, {}, {}", i, j, k
					);
				}
			}
		}

		for i in 0..20
		{
			for j in 0..20
			{
				for k in 0..20
				{
					for l in 0..20
					{
						let path =
							FragmentPath([Some(i), Some(j), Some(k), Some(l)]);
						assert_eq!(
							path.is_disjoint(),
							i != j && i != k && i != l
								&& j != k && j != l
								&& k != l,
							"{}, {}, {}, {}", i, j, k, l
						);
					}
				}
			}
		}
	}

	/// Ensure the correctness of the solution to a canonical puzzle. Only give
	/// the solver 1s to solve the puzzle, which should be sufficient.
	#[test]
	fn test_solver()
	{
		let dictionary = Rc::new(Dictionary::open("dict", "english").unwrap());
		let cases = [
			(
				[
					str8::from("azz"),
					str8::from("th"),
					str8::from("ss"),
					str8::from("tru"),
					str8::from("ref"),
					str8::from("fu"),
					str8::from("ra"),
					str8::from("nih"),
					str8::from("cro"),
					str8::from("mat"),
					str8::from("wo"),
					str8::from("sh"),
					str8::from("re"),
					str8::from("rds"),
					str8::from("tic"),
					str8::from("il"),
					str8::from("lly"),
					str8::from("zz"),
					str8::from("is"),
					str8::from("ment")
				],
				vec![
					str32::from("cross"),
					str32::from("crosswords"),
					str32::from("fully"),
					str32::from("fuss"),
					str32::from("fuzz"),
					str32::from("is"),
					str32::from("mat"),
					str32::from("nihilistic"),
					str32::from("rail"),
					str32::from("rally"),
					str32::from("rare"),
					str32::from("rash"),
					str32::from("razz"),
					str32::from("razzmatazz"),
					str32::from("recross"),
					str32::from("ref"),
					str32::from("refresh"),
					str32::from("refreshment"),
					str32::from("rewords"),
					str32::from("this"),
					str32::from("thrash"),
					str32::from("thresh"),
					str32::from("tic"),
					str32::from("truss"),
					str32::from("truth"),
					str32::from("truthfully"),
					str32::from("words"),
					str32::from("wore")
				]
			),
			(
				[
					str8::from("tab"),
					str8::from("nch"),
					str8::from("ec"),
					str8::from("dis"),
					str8::from("oo"),
					str8::from("per"),
					str8::from("mb"),
					str8::from("ous"),
					str8::from("cour"),
					str8::from("le"),
					str8::from("mar"),
					str8::from("te"),
					str8::from("zle"),
					str8::from("su"),
					str8::from("la"),
					str8::from("ba"),
					str8::from("ket"),
					str8::from("del"),
					str8::from("il"),
					str8::from("chi")
				],
				vec![
					str32::from("bail"),
					str32::from("bale"),
					str32::from("bamboo"),
					str32::from("bamboozle"),
					str32::from("bate"),
					str32::from("chi"),
					str32::from("chinchilla"),
					str32::from("courteous"),
					str32::from("delectable"),
					str32::from("discourteous"),
					str32::from("diskette"),
					str32::from("lamb"),
					str32::from("late"),
					str32::from("leper"),
					str32::from("market"),
					str32::from("per"),
					str32::from("peril"),
					str32::from("perilous"),
					str32::from("super"),
					str32::from("supermarket"),
					str32::from("tab"),
					str32::from("table"),
					str32::from("taboo")
				]
			)
		];
		for (fragments, expected) in cases.iter()
		{
			let solver = Solver::new(Rc::clone(&dictionary), *fragments);
			let solver = solver.solve_fully();
			assert!(solver.is_finished());
			assert!(solver.is_solved());
			let mut solution = solver.solution();
			solution.sort();
			for word in solution.iter()
			{
				assert!(
					dictionary.contains(word.as_str()),
					"not in dictionary: {}",
					word
				);
			}
			let expected = HashSet::<str32>::from_iter(expected.iter().cloned());
			let solution = HashSet::<str32>::from_iter(solution.iter().cloned());
			// The solution may contain additional words, so we only check that
			// the expected words are present. The test dictionary should be
			// capable enough to find the expected solution.
			assert!(expected.is_subset(&solution));
		}
	}
}
