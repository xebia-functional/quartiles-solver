//! # Quartiles Solver
//!
//! Quartiles is a word puzzle game where the player must form words from a
//! 4x5 grid of substrings of 5 long words. To win, the player must form the
//! original 5 long words from the grid. Forming other valid words will earn
//! additional points. Like Boggle and unlike Wordle and Quordle, the complete
//! game board is visible to the player from the start. Combined with
//! proficiency in the English language, the player has complete information
//! about the game state.
//!
//! This program is a solver for the Quartiles game. Via command line options,
//! the user can specify the dictionary to use for solving the puzzle. Then the
//! user can interact with the program via a text-based user interface (TUI) to
//! input the game board and solve the puzzle.

#![allow(uncommon_codepoints)]

mod app;
mod dictionary;
mod solver;
mod tui;

use std::panic;

use clap::{Parser, Subcommand};
use log::{debug, trace};

use tui::tui;
use quartiles_solver::dictionary::Dictionary;

////////////////////////////////////////////////////////////////////////////////
//                           Command line options.                            //
////////////////////////////////////////////////////////////////////////////////

/// CLI for solving Quartiles puzzles.
#[derive(Clone, Debug, Parser)]
#[command(version = "1.0", author = "Todd L Smith")]
struct Opts
{
	/// The path to the directory containing the dictionary files. Can be
	/// changed from the TUI.
	#[arg(short = 'd', long, default_value = "dict")]
	directory: String,

	/// The name of the dictionary. This is the name shared by the text and
	/// binary files, sans the extension. Can be changed from the TUI.
	#[arg(short = 'n', long, default_value = "english")]
	dictionary: String,

	#[command(subcommand)]
	command: Command
}

/// The subcommands of the CLI.
#[derive(Copy, Clone, Debug, Subcommand)]
enum Command
{
	/// Just generate the binary dictionary and exit.
	Generate,

	/// Open the text-based user interface (TUI) for inputting and solving a
	/// Quartiles puzzle. The solution will be written to standard output.
	Solve {
		/// How long (in µs) to highlight an individual word in the TUI.
		#[arg(short = 'd', long, default_value = "400")]
		highlight_duration: u64,

		/// Suppress emission of the solution to standard output.
		#[arg(short = 'q', long)]
		quiet: bool
	}
}

////////////////////////////////////////////////////////////////////////////////
//                               Main program.                                //
////////////////////////////////////////////////////////////////////////////////

/// Parse the command line options and execute the appropriate subcommand.
fn main()
{
	// Parse the command line options.
	let opts = Opts::parse();
	debug!("Command line options: {:?}", opts);

	// Open the dictionary, creating the binary dictionary if necessary.
	let dictionary = Dictionary::open(&opts.directory, &opts.dictionary)
		.unwrap_or_else(|_|
			panic!("Failed to open dictionary: {}/{}.dict or {0}/{1}.txt",
				opts.directory,
				opts.dictionary
			)
		);

	// Execute the appropriate subcommand.
	match opts.command
	{
		Command::Generate =>
		{
			trace!("Exiting after generating binary dictionary");
		},
		Command::Solve { highlight_duration, quiet} =>
		{
			trace!("Opening TUI");
			let solution = tui(highlight_duration, dictionary)
				.unwrap_or_else(|e| panic!("Failed to drive TUI: {}", e));
			if !quiet
			{
				print_solution(solution);
			}
		}
	}
}

/// Print the solution to standard output.
///
/// # Arguments
///
/// * `solution` - The solution to print, as a word list.
fn print_solution(solution: Vec<String>)
{
	for word in solution
	{
		println!("{}", word);
	}
}
