//! # Dictionary
//!
//! Herein is support for dictionary construction and manipulation. All runtime
//! operations are performed against a [`Dictionary`], which is a prefix tree
//! of words.

use std::{
	fs::File,
	io::{self, BufRead, BufReader, ErrorKind, Read, Write},
	path::Path
};

use log::{trace, warn};
use pfx::PrefixTreeSet;
use serde::{Deserialize, Serialize};

////////////////////////////////////////////////////////////////////////////////
//                                Definitions.                                //
////////////////////////////////////////////////////////////////////////////////

/// A dictionary is a [`PrefixTreeSet`] of words.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[must_use]
pub struct Dictionary(PrefixTreeSet<String>);

impl Dictionary
{
	/// Construct an empty dictionary. Same as [`Default::default`].
	///
	/// # Returns
	///
	/// An empty dictionary.
	#[inline]
	pub fn new() -> Self { Self(Default::default()) }

	/// Check if the dictionary is empty.
	///
	/// # Returns
	///
	/// `true` if the dictionary is empty, `false` otherwise.
	#[inline]
	#[must_use]
	pub fn is_empty(&self) -> bool { self.0.is_empty() }

	/// Check if the dictionary contains the given word.
	///
	/// # Arguments
	///
	/// * `word` - The word to check.
	///
	/// # Returns
	///
	/// `true` if the dictionary contains the word, `false` otherwise.
	#[inline]
	#[must_use]
	pub fn contains(&self, word: &str) -> bool { self.0.contains(word) }

	/// Check if the dictionary contains a word with the given prefix.
	///
	/// # Arguments
	///
	/// * `prefix` - The prefix to check.
	///
	/// # Returns
	///
	/// `true` if the dictionary contains a word with the given prefix, `false`
	/// otherwise.
	#[inline]
	#[must_use]
	pub fn contains_prefix(&self, prefix: &str) -> bool
	{
		self.0.contains_prefix(prefix)
	}

	/// Populate the dictionary with the given words.
	///
	/// # Arguments
	///
	/// * `words` - The intended content of the dictionary.
	pub fn populate<T: AsRef<str>>(&mut self, words: &[T])
	{
		for word in words
		{
			self.0.insert(word.as_ref().to_string());
		}
	}

	/// Open a dictionary with the given name. Only the specified directory will
	/// be searched. `name` denotes the dictionary file, sans the extension. If
	/// a binary dictionary (`<name>.dict`) exists _and_ is newer than the text
	/// file (`<name>.txt`), it will be read; otherwise, a text file will be
	/// read and a binary dictionary will be created (to optimize future reads).
	///
	/// # Arguments
	///
	/// * `dir` - The directory to search.
	/// * `name` - The name of the dictionary file.
	///
	/// # Returns
	///
	/// A dictionary containing the words from the file.
	///
	/// # Errors
	///
	/// * If the file cannot be opened or read, an error is returned.
	/// * If the file contains invalid data, an [`ErrKind::InvalidData`] is
	///   returned.
	pub fn open<T: AsRef<Path>>(dir: T, name: &str) -> Result<Self, io::Error>
	{
		let dict_path = dir.as_ref().join(format!("{}.dict", name));
		let txt_path = dir.as_ref().join(format!("{}.txt", name));
		// The possibility of I/O errors makes this rather messy, unfortunately,
		// but the gist is to compare the modification times of the binary and
		// text files in pursuit of using the binary dictionary only if it's
		// newer than the text dictionary. If anything goes wrong, we fall back
		// to reading the text file. Note that we don't have to explicitly
		// check for the existence of the binary dictionary file, as the
		// `metadata` call will fail if it doesn't exist.
		if dict_path
			.metadata()
			.and_then(|m| m.modified())
			.and_then(|dict_time| {
				txt_path
					.metadata()
					.and_then(|n| n.modified())
					.map(|txt_time| dict_time > txt_time)
			})
			.unwrap_or(false)
		{
			let dictionary = Self::deserialize_from_file(&dict_path);
			trace!("Read binary dictionary: {}", dict_path.display());
			dictionary
		}
		else
		{
			let dictionary = Self::read_from_file(&txt_path)?;
			trace!("Read text dictionary: {}", txt_path.display());
			match dictionary.serialize_to_file(&dict_path)
			{
				Ok(_) =>
				{
					trace!("Wrote binary dictionary: {}", dict_path.display())
				},
				Err(e) => warn!(
					"Failed to write binary dictionary: {}: {}",
					dict_path.display(),
					e
				)
			}
			Ok(dictionary)
		}
	}

	/// Construct a dictionary from the contents of the given file. Each line
	/// in the file is considered a single word.
	///
	/// # Arguments
	///
	/// * `path` - The target file.
	///
	/// # Returns
	///
	/// A dictionary containing the words from the file.
	///
	/// # Errors
	///
	/// If the file cannot be opened or read, an error is returned.
	pub fn read_from_file<T: AsRef<Path>>(path: T) -> Result<Self, io::Error>
	{
		let file = File::open(path)?;
		let reader = BufReader::new(file);
		let words =
			reader.lines().map(|line| line.unwrap()).collect::<Vec<_>>();
		let mut dictionary = Self::new();
		dictionary.populate(&words);
		Ok(dictionary)
	}

	/// Deserialize a dictionary from the given file. The file must contain a
	/// serialized dictionary in [`bincode`](bincode) format.
	///
	/// # Arguments
	///
	/// * `path` - The target file.
	///
	/// # Returns
	///
	/// A dictionary deserialized from the file.
	///
	/// # Errors
	///
	/// * If the file cannot be opened or read, an error is returned.
	/// * If the file contains invalid data, an [`ErrKind::InvalidData`] is
	///   returned.
	pub fn deserialize_from_file<T: AsRef<Path>>(
		path: T
	) -> Result<Self, io::Error>
	{
		let file = File::open(path)?;
		let mut reader = BufReader::new(file);
		let mut content = Vec::new();
		reader.read_to_end(&mut content)?;
		let dictionary = bincode::deserialize(&content)
			.map_err(|_e| ErrorKind::InvalidData)?;
		Ok(dictionary)
	}

	/// Serialize the dictionary to the given file. The dictionary is serialized
	/// in [`bincode`](bincode) format.
	///
	/// # Arguments
	///
	/// * `path` - The target file.
	///
	/// # Errors
	///
	/// * If the file cannot be opened or written, an error is returned.
	/// * If the file contains invalid data, an [`ErrKind::InvalidData`] is
	///   returned.
	pub fn serialize_to_file<T: AsRef<Path>>(
		&self,
		path: T
	) -> Result<(), io::Error>
	{
		let mut file = File::create(path)?;
		let content =
			bincode::serialize(self).map_err(|_e| ErrorKind::InvalidData)?;
		file.write_all(&content)?;
		Ok(())
	}
}

////////////////////////////////////////////////////////////////////////////////
//                                   Tests.                                   //
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test
{
	use crate::dictionary::Dictionary;
	use tempfile::NamedTempFile;

	/// The path to the dictionary file.
	#[inline]
	#[must_use]
	const fn test_path() -> &'static str { "dict/english.txt" }

	/// Test basic functionality of [`Dictionary`]:
	///
	/// * [`Dictionary::empty`]
	/// * [`Dictionary::is_empty`]
	/// * [`Dictionary::populate`]
	/// * [`Dictionary::contains`]
	#[test]
	fn test_populate()
	{
		let mut dictionary = Dictionary::new();
		assert!(dictionary.is_empty());
		assert!(!dictionary.contains("hello"));
		assert!(!dictionary.contains("world"));
		dictionary.populate(&["hello", "world"]);
		assert!(dictionary.contains("hello"));
		assert!(dictionary.contains("world"));
	}

	/// Test reading a dictionary from a file:
	///
	/// * [`Dictionary::read_from_file`]
	#[test]
	fn test_read_from_file()
	{
		let dictionary = Dictionary::read_from_file(test_path()).unwrap();
		assert!(!dictionary.is_empty());
		// These words had better be in the dictionary…
		assert!(dictionary.contains("hello"));
		assert!(dictionary.contains("world"));
	}

	/// Test serializing and deserializing a dictionary:
	///
	/// * [`Dictionary::serialize_to_file`]
	#[test]
	fn test_serialize_to_file()
	{
		let dictionary = Dictionary::read_from_file(test_path()).unwrap();
		let file = NamedTempFile::new().unwrap();
		dictionary.serialize_to_file(file.path()).unwrap();
		let deserialized =
			Dictionary::deserialize_from_file(file.path()).unwrap();
		assert_eq!(dictionary, deserialized);
	}
}
