use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use std::io::{stdin, Error, ErrorKind, Result, Write};

const OPTIONS: &str = "[ (y)es, (N)o, yes to (a)ll, (q)uit ]";

#[derive(Clone, Copy)]
pub enum Decision {
    Yes,
    No,
    Quit,
}

#[derive(Clone, Copy, Default)]
pub struct DecisionContext {
    pub is_dry_run: bool,
}

pub trait Decide {
    fn obtain_decision(
        &mut self,
        ctx: &DecisionContext,
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
        ctx: &DecisionContext,
        question: impl AsRef<str>,
    ) -> Result<Decision> {
        let question = question.as_ref();
        println!(
            "{} {}{}: ",
            question,
            OPTIONS,
            if ctx.is_dry_run { " [dry-run]" } else { "" }
        );
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

#[derive(Default)]
pub struct NiceInteractiveDecider {
    decision_memory: Option<Decision>,
}

impl Decide for NiceInteractiveDecider {
    fn obtain_decision(
        &mut self,
        ctx: &DecisionContext,
        question: impl AsRef<str>,
    ) -> Result<Decision> {
        Ok(self.decision_memory.as_ref().copied().unwrap_or_else(|| {
            match Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "{}{}",
                    question.as_ref(),
                    if ctx.is_dry_run { " [dry-run]" } else { "" }
                ))
                .default(false)
                .show_default(true)
                .wait_for_newline(false)
                .interact_opt()
                .unwrap()
            {
                None => Decision::Quit,
                Some(true) => Decision::Yes,
                Some(false) => Decision::No,
            }
        }))
    }
}
