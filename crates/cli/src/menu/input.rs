use std::io::{self, BufRead, Write};
pub(super) fn ask(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    prompt: &str,
) -> io::Result<Option<String>> {
    write!(writer, "{prompt}")?;
    writer.flush()?;
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}
