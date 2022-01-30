use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use std::io::Result;

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
