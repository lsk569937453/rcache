use std::collections::Bound;
use std::str::from_utf8;

use anyhow::anyhow;
use std::f64::{INFINITY, NEG_INFINITY};

/// A command argument
#[derive(Debug, Clone)]
pub struct Argument {
    /// The position in the array
    pub pos: usize,
    /// The length in the array
    pub len: usize,
}

/// A protocol parser
pub struct ParsedCommand {
    /// The data itself
    data: Vec<u8>,
    /// The arguments location and length
    pub argv: Vec<Argument>,
}
impl ParsedCommand {
    /// Creates a new parser with the data and arguments provided
    pub fn new(data: Vec<u8>, argv: Vec<Argument>) -> ParsedCommand {
        ParsedCommand { data, argv }
    }
    /// Gets a `Bound` from a parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::collections::Bound;
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"+inf", vec![Argument { pos: 0, len: 4 }]);
    /// assert_eq!(parser.get_f64_bound(0).unwrap(), Bound::Unbounded);
    /// ```
    ///
    /// ```
    /// # use std::collections::Bound;
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"1.23", vec![Argument { pos: 0, len: 4 }]);
    /// assert_eq!(parser.get_f64_bound(0).unwrap(), Bound::Included(1.23));
    /// ```
    ///
    /// ```
    /// # use std::collections::Bound;
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"(1.23", vec![Argument { pos: 0, len: 5 }]);
    /// assert_eq!(parser.get_f64_bound(0).unwrap(), Bound::Excluded(1.23));
    /// ```
    pub fn get_f64_bound(&self, pos: usize) -> Result<Bound<f64>, anyhow::Error> {
        let s = self.get_str(pos)?;
        if s == "inf" || s == "+inf" || s == "-inf" {
            return Ok(Bound::Unbounded);
        }

        if s.starts_with('(') {
            let f = s[1..].parse::<f64>()?;
            if f.is_nan() {
                return Err(anyhow!("InvalidArgument"));
            }
            return Ok(Bound::Excluded(f));
        }
        let f = s.parse::<f64>()?;

        if f.is_nan() {
            Err(anyhow!("InvalidArgument"))
        } else {
            Ok(Bound::Included(f))
        }
    }

    // TODO: get<T>

    /// Gets an f64 from a parameter
    ///
    /// # Examples
    ///
    /// ```
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"1.23", vec![Argument { pos: 0, len: 4 }]);
    /// assert_eq!(parser.get_f64(0).unwrap(), 1.23);
    /// ```
    pub fn get_f64(&self, pos: usize) -> Result<f64, anyhow::Error> {
        let s = self.get_str(pos)?;
        if s == "+inf" || s == "inf" {
            return Ok(INFINITY);
        }
        if s == "-inf" {
            return Ok(NEG_INFINITY);
        }
        let f = s.parse::<f64>()?;
        if f.is_nan() {
            Err(anyhow!("InvalidArgument"))
        } else {
            Ok(f)
        }
    }

    /// Gets an i64 from a parameter
    ///
    /// # Examples
    ///
    /// ```
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"-123", vec![Argument { pos: 0, len: 4 }]);
    /// assert_eq!(parser.get_i64(0).unwrap(), -123);
    /// ```
    pub fn get_i64(&self, pos: usize) -> Result<i64, anyhow::Error> {
        let s = self.get_str(pos)?;

        Ok(s.parse::<i64>()?)
    }

    /// Gets an str from a parameter
    ///
    /// # Examples
    ///
    /// ```
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"foo", vec![Argument { pos: 0, len: 3 }]);
    /// assert_eq!(parser.get_str(0).unwrap(), "foo");
    /// ```
    pub fn get_str(&self, pos: usize) -> Result<&str, anyhow::Error> {
        let data = self.get_slice(pos)?;
        Ok(from_utf8(data)?)
    }

    /// Gets a Vec<u8> from a parameter
    ///
    /// # Examples
    ///
    /// ```
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"foo", vec![Argument { pos: 0, len: 3 }]);
    /// assert_eq!(parser.get_vec(0).unwrap(), b"foo".to_vec());
    /// ```
    pub fn get_vec(&self, pos: usize) -> Result<Vec<u8>, anyhow::Error> {
        let data = self.get_slice(pos)?;
        Ok(data.to_vec())
    }

    /// Gets a &[u8] from a parameter
    ///
    /// # Examples
    ///
    /// ```
    /// # use parser::{ParsedCommand, Argument};
    /// let parser = ParsedCommand::new(b"foo", vec![Argument { pos: 0, len: 3 }]);
    /// assert_eq!(parser.get_slice(0).unwrap(), b"foo");
    /// ```
    pub fn get_slice(&self, pos: usize) -> Result<&[u8], anyhow::Error> {
        if pos >= self.argv.len() {
            return Err(anyhow!("InvalidArgument"));
        }
        let arg = &self.argv[pos];
        Ok(&self.data[arg.pos..arg.pos + arg.len])
    }

    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }
}
