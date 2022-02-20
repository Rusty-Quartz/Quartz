use std::io::{Read, Write};

/// A list of commands loaded from a mcfunction file
///
/// This is just the contents of the file but with comments stripped
pub type Function = Vec<String>;

/// Reads the lines of a string from `reader` and returns a Function after stripping the comments
pub fn read_function<T: Read>(mut reader: T) -> std::io::Result<Function> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;

    Ok(buf
        .lines()
        .filter_map(|l| {
            // Filter out all comment lines
            if !l.starts_with('#') {
                Some(l.to_owned())
            } else {
                None
            }
        })
        .collect())
}

pub fn write_function<T: Write>(function: &Function, mut writer: T) -> std::io::Result<()> {
    write!(writer, "{}", function.join("\n"))
}
