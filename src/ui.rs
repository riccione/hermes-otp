use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use console::{Term};
use crate::cmd::print_table;
use crate::models::Record;
use crate::otp;

pub(crate) struct Table<'a> {
    filtered: &'a[&'a Record],
    pass: &'a String,
    is_single_alias: bool
}

impl<'a> Table<'a> {
    pub fn new(filtered: &'a [&Record], pass: &'a String, is_single_alias: bool) -> Self {
        Table {
            filtered,
            pass,
            is_single_alias
        }
    }
    pub fn render(&self) {
        let output = Term::stdout();
        let err = Term::stderr();
        output.hide_cursor().unwrap();

        let flag = Arc::new(AtomicBool::new(false));

        let flag_ref = flag.clone();
        let term_ref = output.clone();
        thread::spawn(move || {
            term_ref.read_key().unwrap();
            flag_ref.store(true, Ordering::Relaxed);
        });

        let mut rem = otp::get_remaining_seconds();
        print_table(self.filtered, self.pass, rem, self.is_single_alias, false);
        eprintln!("Press any key to exit");
        err.move_cursor_up(1).unwrap();

        while flag.load(Ordering::Relaxed) == false {
            rem = otp::get_remaining_seconds();
            if self.is_single_alias & (self.filtered.len() == 1) {
                output.move_cursor_up(1).unwrap();
                err.move_cursor_up(1).unwrap();
            } else {
                output.move_cursor_up(self.filtered.len() + 2).unwrap(); // One line per record plus 2 lines for table headers
            }
            print_table(self.filtered, self.pass, rem, self.is_single_alias, false);
            sleep(Duration::from_millis(100));
        }
    }
}

impl<'a> Drop for Table<'a> {
    fn drop(&mut self) { // Safe exit (also works in case of panic)
        let term = Term::stdout();
        term.show_cursor().unwrap();
        term.move_cursor_down(f64::INFINITY as usize).unwrap();
        term.clear_last_lines(1).unwrap() // Clear "press any key to exit" line
    }
}