use crate::Logger;
use std::io::{stdin, Error, ErrorKind, Result, Write};

const OPTIONS: &str = "[ (y)es, (N)o, yes to (a)ll, (q)uit ]";

#[derive(Clone, Copy)]
pub enum Decision {
    Yes,
    No,
    Quit,
}

pub trait Decide {
    fn obtain_decision(
        &mut self,
        logger: &dyn Logger,
        question: impl AsRef<str>,
    ) -> Result<Decision>;
}

#[derive(Default)]
pub struct InteractiveDecisionWithMemory {
    decision_memory: Option<Decision>,
}

impl Decide for InteractiveDecisionWithMemory {
    fn obtain_decision(
        &mut self,
        logger: &dyn Logger,
        question: impl AsRef<str>,
    ) -> Result<Decision> {
        let question = question.as_ref();
        logger.question(format!("{} {}: ", question, OPTIONS).as_str());
        std::io::stdout().flush()?;
        if self.decision_memory.is_some() {
            println!("yes (from previous choice)");
            std::io::stdout().flush()?;

            return self
                .decision_memory
                .ok_or_else(|| Error::from(ErrorKind::Other));
        }

        let mut decision = String::new();
        stdin().read_line(&mut decision)?;
        match decision.trim() {
            "y" => Ok(Decision::Yes),
            "a" => {
                self.decision_memory = Some(Decision::Yes);
                self.decision_memory
                    .ok_or_else(|| Error::from(ErrorKind::Other))
            }
            "q" => Ok(Decision::Quit),
            _ => Ok(Decision::No),
        }
    }
}
