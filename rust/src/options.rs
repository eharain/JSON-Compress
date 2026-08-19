//! What the encoder is allowed to do.

use crate::error::{Error, Result};

/// The transformations the encoder may apply, and the thresholds it applies
/// them at.
///
/// [`Options::default`] is the tuned set - every switch on, thresholds picked
/// for API responses and log batches, which is where the format earns most of
/// its keep - so reach for it and change what you need:
///
/// ```
/// use json_compress::Options;
///
/// let options = Options {
///     min_rows: 10,
///     ..Options::default()
/// };
/// ```
///
/// Every default is safe on arbitrary input. Nothing here changes what a decoder
/// has to do: two encoders may make different choices and both produce valid
/// documents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// De-duplicate repeated string values into a catalogue.
    pub strings: bool,
    /// Pack arrays of like-shaped objects into columnar tables.
    pub tables: bool,
    /// Hoist columns whose value never changes out of the rows.
    pub constants: bool,
    /// Encode low-cardinality string columns as integer indices.
    pub dict_columns: bool,
    /// The smallest array that may become a table.
    pub min_rows: usize,
    /// The largest share of missing cells a table may carry, from 0 to 1.
    pub max_hole_ratio: f64,
    /// How many times a string must occur before it can be catalogued.
    pub min_string_count: usize,
    /// The shortest string that may be catalogued, in UTF-16 code units.
    pub min_string_length: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            strings: true,
            tables: true,
            constants: true,
            dict_columns: true,
            min_rows: 2,
            max_hole_ratio: 0.3,
            min_string_count: 2,
            min_string_length: 1,
        }
    }
}

impl Options {
    /// Checks that the options make sense, so that a typo surfaces at the call
    /// site rather than as a mysteriously large output.
    pub fn validate(&self) -> Result<()> {
        if self.min_rows < 1 {
            return Err(Error::options(
                "min_rows must be at least 1 - build options from Options::default()",
            ));
        }
        if !(0.0..=1.0).contains(&self.max_hole_ratio) {
            return Err(Error::options(format!(
                "max_hole_ratio must be between 0 and 1, not {}",
                self.max_hole_ratio
            )));
        }
        if self.min_string_count < 2 {
            return Err(Error::options(format!(
                "min_string_count must be at least 2, not {}",
                self.min_string_count
            )));
        }
        Ok(())
    }
}
