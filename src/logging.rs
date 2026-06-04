use std::fmt::Display;
use std::io::{self, Write};

pub fn stdout_line(message: impl Display) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{message}");
}

pub fn stderr_line(message: impl Display) {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "{message}");
}
