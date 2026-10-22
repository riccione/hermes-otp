use crate::cmd::{Row, print_table};
use console::Term;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) struct Table<'a, 'b> {
    rows: &'a [Row<'b>],
    is_single_alias: bool,
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
        let _ = output.hide_cursor();

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

            if self.is_single_alias && self.rows.len() == 1 {
                let _ = output.move_cursor_up(1);
                let _ = err.move_cursor_up(1);
            } else {
                let _ = output.move_cursor_up(self.rows.len() + 2);
            }
            print_table(self.rows, self.is_single_alias, false);
        }
    }
}

impl<'a, 'b> Drop for Table<'a, 'b> {
    fn drop(&mut self) {
        // Safe exit (also works in case of panic)
        let term = Term::stdout();
        term.show_cursor().unwrap();
        term.move_cursor_down(usize::MAX).unwrap();
        term.clear_last_lines(1).unwrap() // Clear "press any key to exit" line
    }
}
