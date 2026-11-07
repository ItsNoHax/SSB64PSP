//! The installer's screen: rust-psp's debug font (`psp::dprint!`, 27 rows
//! of 80 columns), no GU and no framebuffer beyond the one VRAM page it
//! draws to. Each frame is printed whole, exactly [`ROWS`] lines each
//! ending in `\n`, so the debug console's scrolling puts the frame's first
//! line at the top every time.

use core::fmt::{self, Write};

/// The debug console's rows (272 / 10).
pub const ROWS: usize = 27;
/// Columns used: under the console's 80, which wraps at 80.
pub const COLS: usize = 78;

/// One frame of text, built without allocating.
pub struct Frame {
    buf: [u8; ROWS * (COLS + 1)],
    len: usize,
    rows: usize,
    /// Columns in the current row.
    col: usize,
}

impl Frame {
    pub const fn new() -> Self {
        Self {
            buf: [0; ROWS * (COLS + 1)],
            len: 0,
            rows: 0,
            col: 0,
        }
    }

    fn end_row(&mut self) {
        if self.rows < ROWS {
            self.buf[self.len] = b'\n';
            self.len += 1;
            self.rows += 1;
        }
        self.col = 0;
    }

    fn put(&mut self, c: u8) {
        if c == b'\n' {
            self.end_row();
            return;
        }
        if self.col == COLS {
            self.end_row();
        }
        if self.rows < ROWS {
            self.buf[self.len] = if c.is_ascii_graphic() || c == b' ' {
                c
            } else {
                b'?'
            };
            self.len += 1;
            self.col += 1;
        }
    }

    /// One line (wrapped at [`COLS`]; `\n` inside starts new rows).
    pub fn line(&mut self, args: fmt::Arguments<'_>) {
        let _ = self.write_fmt(args);
        self.end_row();
    }

    /// An empty line.
    pub fn blank(&mut self) {
        self.end_row();
    }

    /// Blank rows up to row `row` (0-based), for a footer at the bottom.
    pub fn pad_to(&mut self, row: usize) {
        if self.col > 0 {
            self.end_row();
        }
        while self.rows < row.min(ROWS) {
            self.end_row();
        }
    }

    /// The frame on screen: padded to [`ROWS`] rows, or to `rows` rows
    /// when another printer adds the rest (the fatal screen).
    pub fn show_rows(&mut self, rows: usize) {
        self.pad_to(rows);
        let text = core::str::from_utf8(&self.buf[..self.len]).unwrap_or("");
        psp::dprint!("{}", text);
    }

    pub fn show(&mut self) {
        self.show_rows(ROWS);
    }
}

impl Write for Frame {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &c in s.as_bytes() {
            self.put(c);
        }
        Ok(())
    }
}

/// `width` columns of bar for `per_mille`.
pub fn bar(f: &mut Frame, per_mille: u32, width: usize) {
    let full = (per_mille.min(1000) as usize * width) / 1000;
    let _ = f.write_char('[');
    for i in 0..width {
        let _ = f.write_char(if i < full { '#' } else { '.' });
    }
    let _ = f.write_char(']');
}

/// `us` microseconds as `m:ss`; honours the format's width and alignment.
pub struct Clock(pub u64);

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.0 / 1_000_000;
        let (m, s) = (s / 60, s % 60);
        // "m:ss" in a stack buffer, then padded as one string.
        let mut buf = [0u8; 24];
        let mut at = buf.len();
        for d in [s % 10, s / 10] {
            at -= 1;
            buf[at] = b'0' + d as u8;
        }
        at -= 1;
        buf[at] = b':';
        let mut m = m;
        loop {
            at -= 1;
            buf[at] = b'0' + (m % 10) as u8;
            m /= 10;
            if m == 0 {
                break;
            }
        }
        f.pad(core::str::from_utf8(&buf[at..]).unwrap_or("?"))
    }
}
