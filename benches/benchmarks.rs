use std::{rc::Rc, time::Duration};

use const_format::concatcp;
use criterion::{measurement::Measurement, BenchmarkGroup, Criterion};
use fixedstr::str8;
use quartiles_solver::{dictionary::Dictionary, solver::Solver};

/// The path of the directory containing the dictionaries.
#[inline]
#[must_use]
const fn dir() -> &'static str
{
	"dict"
}

/// The name of the dictionary file.
#[inline]
#[must_use]
const fn name() -> &'static str
{
	"english"
}

/// The path to the text file.
#[inline]
#[must_use]
const fn path_txt() -> &'static str
{
	concatcp!(dir(), "/", name(), ".txt")
}

/// The path to the text file.
#[inline]
#[must_use]
const fn path_dict() -> &'static str
{
	concatcp!(dir(), "/", name(), ".dict")
}

/// Benchmark reading a dictionary from a file.
///
/// # Arguments
///
/// * `g` - The benchmark group.
fn bench_read_from_file<M: Measurement>(g: &mut BenchmarkGroup<M>)
{
	g.bench_function("read_from_file", |b| {
		b.iter(|| Dictionary::read_from_file(path_txt()).unwrap());
	});
}

/// Benchmark deserializing a dictionary from a file.
///
/// # Arguments
///
/// * `g` - The benchmark group.
fn bench_deserialize_from_file<M: Measurement>(g: &mut BenchmarkGroup<M>)
{
	g.bench_function("deserialize_from_file", |b| {
		b.iter(|| Dictionary::deserialize_from_file(path_dict()).unwrap());
	});
}

/// Benchmark solving a puzzle.
///
/// # Arguments
///
/// * `g` - The benchmark group.
fn bench_solver<M: Measurement>(g: &mut BenchmarkGroup<M>)
{
	g.bench_function("solve", |b| {
		b.iter(|| {
			let dictionary = Rc::new(Dictionary::open(dir(), name()).unwrap());
			let fragments = [
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
			];
			let solver = Solver::new(dictionary, fragments);
			// 10s should be vastly more than enough time to solve the puzzle.
			let solver = solver.solve_fully();
			assert!(solver.is_solved());
		});
	});
}

/// Run all benchmarks.
///
/// The main purpose of the benchmarking is to ensure that
/// [`deserialize_from_file`](Dictionary::deserialize_from_file) is faster than
/// [`read_from_file`](Dictionary::read_from_file).
fn main()
{
	// Ensure that both the text and binary files exist.
	let _ = Dictionary::open(dir(), name()).unwrap();

	// Run the benchmarks.
	let mut criterion = Criterion::default().configure_from_args();
	let mut group = criterion.benchmark_group("benchmarks");
	group.measurement_time(Duration::from_secs(30));
	bench_read_from_file(&mut group);
	bench_deserialize_from_file(&mut group);
	bench_solver(&mut group);
	group.finish();

	// Generate the final summary.
	criterion.final_summary();
}
