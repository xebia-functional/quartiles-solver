Quartiles Solver
----------------

Herein is a high-performance solver for Apple News+ game
[Quartiles](https://www.apple.com/newsroom/2024/05/apple-news-plus-introduces-quartiles-a-new-game-and-offline-mode-for-subscribers/),
written in [Rust](https://www.rust-lang.org/) using the
[Ratatui](https://ratatui.rs/) text-based user interface (TUI) library.

Modes
-----

There are two modes of operation: `generate` and `solve`. The mode is specified
by eponymous subcommand.

In both modes, the application looks for an English dictionary in the directory
specified via the `-d` option, which defaults to [`dict`](dict) if unspecified.
The name of the dictionary, sans the file extension, is specified via the `-n`
option, which defaults to `english` if unspecified. If a binary dictionary
(`.dict`) is present, then the application uses it; otherwise, the plaintext
dictionary (`.txt`) is used instead, and an eponymous binary dictionary
(`.dict`) is generated next to the plaintext one.

In `generate` mode, the application exits after performing the conversion.

In `solve` mode, the application opens the TUI:

![Initial TUI](blog/Quartiles%20Solver%20Start.png)

The user may then fill in the Quartiles board by navigating among the cells. The
following commands are available:

* Up arrow: Select the cell above.
* Right arrow: Select the cell to the right.
* Down arrow: Select the cell below.
* Left arrow: Select the cell to the left.
* Tab: Select the next cell, iterating left-to-right and wrapping to the
  beginning of the row below.
* Shift+Tab: Select the previous cell, iterating right-to-left and wrapping to
  the end of the row above.
* Delete: Clear the selected cell.
* Shift+Delete: Clear all cells.
* A, B, C, D, …, X, Y, Z: Append the corresponding letter to the selected cell.
* Backspace: Remove the last letter from the selected cell.
* Escape: Exit the program.
* Enter: Start the solver. Requires every cell to be populated. No effect if
  any cells remain empty.

After filling in a board, it should look something like this:

![Filled TUI](blog/Quartiles%20Solver%20Ready.png)

The user may then press Enter to launch the solver. The solver animates its
traversal of the search space, highlighting valid words and adding them to the
Solution:

![Running the solver](blog/Quartiles%20Solver%20Running.png)

While the solver is running, the user may press Escape to exit the program. When
the solver completes, an indication of success or failure appears along the
bottom edge of the Puzzle pane, and focus moves to the Solution pane.

![Solution found](blog/Quartiles%20Solver%20Solved.png)

The following commands are available:

* Up arrow: Select the word above. Deselects at the top edge.
* Down arrow: Select the word below. Deselects at the bottom edge.
* Escape: Exit the program.

After the TUI exits, the terminal is restored and the complete solution is
written to standard output (unless the `-q` option is used).

Building
--------

```shell
$ cargo build --release
```

Running
-------

In `generate` mode:

```shell
$ cargo run --release generate
```

In `solve` mode:

```shell
$ cargo run --release solve
```

Command Line Arguments
----------------------

To display the main modes and general options:

```text
at 21:36:01 ➜ cargo run --release -- --help
CLI for solving Quartiles puzzles

Usage: quartiles-solver [OPTIONS] <COMMAND>

Commands:
  generate  Just generate the binary dictionary and exit
  solve     Open the text-based user interface (TUI) for inputting and solving a Quartiles puzzle. The solution will be written to standard output
  help      Print this message or the help of the given subcommand(s)

Options:
  -d, --directory <DIRECTORY>    The path to the directory containing the dictionary files. Can be changed from the TUI [default: dict]
  -n, --dictionary <DICTIONARY>  The name of the dictionary. This is the name shared by the text and binary files, sans the extension. Can be changed from the TUI [default: english]
  -h, --help                     Print help
  -V, --version                  Print version
```

When running the application in `generate` mode, no special options are
recognized.

When running the application in `solve` mode, the follow options are recognized:

```text
at 21:35:21 ➜ cargo run --release solve --help
Open the text-based user interface (TUI) for inputting and solving a Quartiles
puzzle. The solution will be written to standard output

Usage: quartiles-solver solve [OPTIONS]

Options:
  -d, --highlight-duration <HIGHLIGHT_DURATION>
          How long (in µs) to highlight an individual word in the TUI [default: 400]
  -q, --quiet
          Suppress emission of the solution to standard output
  -h, --help
          Print help
```
