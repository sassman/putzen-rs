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
    pub yes_to_all: bool,
}

impl DecisionContext {
    pub fn println(&self, msg: impl AsRef<str>) {
        println!("{}", msg.as_ref());
    }
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
        let suffix = if ctx.is_dry_run { " [dry-run]" } else { "" };
        Ok(self.decision_memory.as_ref().copied().unwrap_or_else(|| {
            if ctx.yes_to_all {
                ctx.println(format!("  {}{suffix} [yes by -y arg]", question.as_ref()));
                Decision::Yes
            } else {
                Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(format!("{}{suffix}", question.as_ref()))
                    .default(false)
                    .show_default(true)
                    .wait_for_newline(false)
                    .interact_opt()
                    .unwrap()
                    .map(|x| if x { Decision::Yes } else { Decision::No })
                    .unwrap_or(Decision::Quit)
            }
        }))
    }
}
