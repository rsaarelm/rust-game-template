use engine::prelude::*;

#[derive(Copy, Clone, Debug)]
pub enum Command {
    Direct(Action),
    Indirect(Goal),
}

impl From<Action> for Command {
    fn from(value: Action) -> Self {
        Command::Direct(value)
    }
}

impl From<Goal> for Command {
    fn from(value: Goal) -> Self {
        Command::Indirect(value)
    }
}
