pub trait Logger {
    fn info(&self, msg: &str);
    fn question(&self, question: &str);
}

#[derive(Default)]
pub struct StdLogger;
impl Logger for StdLogger {
    fn info(&self, msg: &str) {
        println!("{}", msg)
    }

    fn question(&self, question: &str) {
        print!("{}", question)
    }
}

#[derive(Default)]
pub struct DryRunLogger(StdLogger);
// impl Default for DryRunLogger {
//     fn default() -> Self {
//         DryRunLogger(StdLogger::default())
//     }
// }
impl Logger for DryRunLogger {
    fn info(&self, msg: &str) {
        print!("[dry-run] ");
        self.0.info(msg)
    }

    fn question(&self, question: &str) {
        print!("[dry-run] ");
        self.0.question(question)
    }
}
