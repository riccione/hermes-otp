use crate::cmd::{Row, print_table};
use console::{Key, Term};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) struct Table<'a, 'b> {
    rows: &'a [Row<'b>],
    is_single_alias: bool,
}

/// Whether the inline view can redraw without scrolling the table off screen.
fn table_fits(height: u16, content_lines: usize) -> bool {
    content_lines + 2 <= height as usize
}

/// Rows visible in the pager (header, separator and footer take four lines).
fn page_size(height: u16) -> usize {
    height.saturating_sub(4).max(1) as usize
}

fn clamp_offset(offset: usize, len: usize, page: usize) -> usize {
    offset.min(len.saturating_sub(page))
}

impl<'a, 'b> Table<'a, 'b> {
    pub fn new(rows: &'a [Row<'b>], is_single_alias: bool) -> Self {
        Table {
            rows,
            is_single_alias,
        }
    }

    pub fn render(&self) {
        let output = Term::stdout();
        let err = Term::stderr();

        let single = self.is_single_alias && self.rows.len() == 1;
        let content_lines = if single { 2 } else { self.rows.len() + 2 };
        let (height, _width) = output.size();

        let _ = output.hide_cursor();

        if !output.is_term() {
            print_table(self.rows, self.is_single_alias, false);
            eprintln!("Press any key to exit");
            return;
        }

        if !single && !table_fits(height, content_lines) {
            self.render_pager(&output);
            return;
        }

        let (tx, rx) = mpsc::channel();
        let term_ref = output.clone();

        thread::spawn(move || {
            if term_ref.read_key().is_ok() {
                let _ = tx.send(());
            }
        });

        print_table(self.rows, self.is_single_alias, false);
        eprintln!("Press any key to exit");
        let _ = err.move_cursor_up(1);

        loop {
            // Redraw once per second. Records may use different periods, so
            // there is no single rollover to wait for; a steady 1s refresh is
            // both cheap and correct for any mix of 30s and 60s records.
            match rx.recv_timeout(Duration::from_secs(1)) {
                // A key was pressed! Exit immediately.
                Ok(_) => break,

                // The background thread died/disconnected (e.g., non-interactive terminal).
                // Exit silently instead of looping forever.
                Err(mpsc::RecvTimeoutError::Disconnected) => break,

                // Just a normal timeout. Redraw.
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            if single {
                let _ = output.move_cursor_up(1);
                let _ = err.move_cursor_up(1);
            } else {
                let _ = output.move_cursor_up(self.rows.len() + 2);
            }
            print_table(self.rows, self.is_single_alias, false);
        }
    }

    /// Full-screen pager for tables taller than the terminal. Keeps refreshing
    /// the visible window every second while the user scrolls.
    fn render_pager(&self, output: &Term) {
        let (tx, rx) = mpsc::channel();
        let term_ref = output.clone();

        thread::spawn(move || {
            while let Ok(key) = term_ref.read_key() {
                if key == Key::Unknown || tx.send(key).is_err() {
                    break;
                }
            }
        });

        let mut offset = 0;

        loop {
            let (height, _width) = output.size();
            let page = page_size(height);
            let len = self.rows.len();
            offset = clamp_offset(offset, len, page);
            let end = (offset + page).min(len);

            let _ = output.clear_screen();
            let _ = output.move_cursor_to(0, 0);
            print_table(&self.rows[offset..end], false, false);
            let _ = output.clear_line();
            println!(
                "{}-{} of {}  Up/Down scroll  PgUp/PgDn  q quit",
                offset + 1,
                end,
                len
            );

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Key::ArrowUp) => offset = offset.saturating_sub(1),
                Ok(Key::ArrowDown) => offset = (offset + 1).min(len.saturating_sub(page)),
                Ok(Key::PageUp) => offset = offset.saturating_sub(page),
                Ok(Key::PageDown) => offset = (offset + page).min(len.saturating_sub(page)),
                Ok(Key::Home) => offset = 0,
                Ok(Key::End) => offset = len.saturating_sub(page),
                Ok(Key::Escape) | Ok(Key::CtrlC) | Ok(Key::Char('q')) => break,
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let _ = output.show_cursor();
    }
}

impl Drop for Table<'_, '_> {
    fn drop(&mut self) {
        // Best-effort cleanup: must never panic, especially during unwinding.
        let term = Term::stdout();
        let _ = term.show_cursor();
        let _ = term.move_cursor_down(usize::MAX);
        let _ = term.clear_last_lines(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_offset, page_size, table_fits};

    #[test]
    fn table_fit_boundary() {
        assert!(table_fits(10, 8));
        assert!(!table_fits(10, 9));
        assert!(table_fits(4, 2));
        assert!(!table_fits(4, 3));
    }

    #[test]
    fn pager_window_boundaries() {
        assert_eq!(page_size(10), 6);
        assert_eq!(page_size(4), 1);
        assert_eq!(page_size(1), 1);

        assert_eq!(clamp_offset(0, 50, 20), 0);
        assert_eq!(clamp_offset(30, 50, 20), 30);
        assert_eq!(clamp_offset(40, 50, 20), 30);
        assert_eq!(clamp_offset(10, 5, 20), 0);
    }
}
