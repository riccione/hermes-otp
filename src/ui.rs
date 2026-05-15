use crate::cmd::print_table;
use crate::models::Record;
use crate::otp;
use console::Term;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(crate) struct Table<'a> {
    filtered: &'a [&'a Record],
    pass: &'a String,
    is_single_alias: bool,
}

impl<'a> Table<'a> {
    pub fn new(filtered: &'a [&Record], pass: &'a String, is_single_alias: bool) -> Self {
        Table {
            filtered,
            pass,
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

        let mut rem = otp::get_remaining_seconds();
        print_table(self.filtered, self.pass, rem, self.is_single_alias, false);
        eprintln!("Press any key to exit");
        let _ = err.move_cursor_up(1);

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                // A key was pressed! Exit immediately.
                Ok(_) => break,

                // The background thread died/disconnected (e.g., non-interactive terminal).
                // Exit silently instead of looping forever.
                Err(mpsc::RecvTimeoutError::Disconnected) => break,

                // Just a normal 100ms timeout. Update the timer and redraw.
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            let new_rem = otp::get_remaining_seconds();

            if new_rem != rem {
                rem = new_rem;

                if self.is_single_alias && self.filtered.len() == 1 {
                    let _ = output.move_cursor_up(1);
                    let _ = err.move_cursor_up(1);
                } else {
                    let _ = output.move_cursor_up(self.filtered.len() + 2);
                }
                print_table(self.filtered, self.pass, rem, self.is_single_alias, false);
            }
        }
    }
}

impl<'a> Drop for Table<'a> {
    fn drop(&mut self) {
        // Safe exit (also works in case of panic)
        let term = Term::stdout();
        term.show_cursor().unwrap();
        term.move_cursor_down(usize::MAX).unwrap();
        term.clear_last_lines(1).unwrap() // Clear "press any key to exit" line
    }
}
