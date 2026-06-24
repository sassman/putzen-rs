//! Pure aggregator returned by `update`. Reimplements the pattern of
//! `crux_core::command::Command<Effect, Event>` with no external dependency.
//!
//! `events` are synchronous Msgs to feed back into `update` in the same tick.
//! `effects` are IO descriptions handed to the runtime's effect runner.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command<E, M> {
    pub events: Vec<M>,
    pub effects: Vec<E>,
}

impl<E, M> Command<E, M> {
    pub fn done() -> Self {
        Self { events: Vec::new(), effects: Vec::new() }
    }

    pub fn event(m: M) -> Self {
        Self { events: vec![m], effects: Vec::new() }
    }

    pub fn effect(e: E) -> Self {
        Self { events: Vec::new(), effects: vec![e] }
    }

    pub fn batch<I: IntoIterator<Item = Self>>(cmds: I) -> Self {
        let mut out = Self::done();
        for c in cmds {
            out.events.extend(c.events);
            out.effects.extend(c.effects);
        }
        out
    }

    pub fn and(mut self, other: Self) -> Self {
        self.events.extend(other.events);
        self.effects.extend(other.effects);
        self
    }

    pub fn is_done(&self) -> bool {
        self.events.is_empty() && self.effects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone)]
    enum E { A, B }
    #[derive(Debug, PartialEq, Eq, Clone)]
    enum M { X, Y }

    #[test]
    fn done_is_empty() {
        let c: Command<E, M> = Command::done();
        assert!(c.is_done());
    }

    #[test]
    fn event_carries_only_msg() {
        let c: Command<E, M> = Command::event(M::X);
        assert_eq!(c.events, vec![M::X]);
        assert!(c.effects.is_empty());
        assert!(!c.is_done());
    }

    #[test]
    fn effect_carries_only_effect() {
        let c: Command<E, M> = Command::effect(E::A);
        assert!(c.events.is_empty());
        assert_eq!(c.effects, vec![E::A]);
    }

    #[test]
    fn and_concatenates_in_order() {
        let c: Command<E, M> = Command::event(M::X).and(Command::effect(E::A));
        assert_eq!(c.events, vec![M::X]);
        assert_eq!(c.effects, vec![E::A]);

        let d: Command<E, M> = Command::effect(E::B).and(Command::event(M::Y));
        assert_eq!(d.events, vec![M::Y]);
        assert_eq!(d.effects, vec![E::B]);
    }

    #[test]
    fn batch_concatenates_all() {
        let c: Command<E, M> = Command::batch([
            Command::event(M::X),
            Command::effect(E::A),
            Command::event(M::Y),
            Command::effect(E::B),
        ]);
        assert_eq!(c.events, vec![M::X, M::Y]);
        assert_eq!(c.effects, vec![E::A, E::B]);
    }

    #[test]
    fn batch_of_empty_is_done() {
        let c: Command<E, M> = Command::batch(std::iter::empty());
        assert!(c.is_done());
    }
}
