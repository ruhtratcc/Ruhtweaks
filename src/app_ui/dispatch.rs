use std::collections::VecDeque;

/// One native wakeup per pending batch. Progress replaces progress; control
/// events are kept in order. No worker or timer exists just to check this queue.
pub struct Inbox<E> {
    pending: VecDeque<E>,
    scheduled: bool,
}
impl<E> Default for Inbox<E> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            scheduled: false,
        }
    }
}
impl<E> Inbox<E> {
    pub fn post(&mut self, value: E, replace: impl FnOnce(&E, &E) -> bool) -> bool {
        if self
            .pending
            .back()
            .is_some_and(|previous| replace(previous, &value))
        {
            *self.pending.back_mut().expect("last entry exists") = value;
        } else {
            self.pending.push_back(value);
        }
        let notify = !self.scheduled;
        self.scheduled = true;
        notify
    }
    pub fn take(&mut self) -> VecDeque<E> {
        self.scheduled = false;
        std::mem::take(&mut self.pending)
    }
    pub fn unschedule(&mut self) {
        self.scheduled = false;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_flood_schedules_once_without_losing_completion() {
        let mut inbox = Inbox::default();
        let mut wakeups = 0;
        for value in 0..100_000 {
            wakeups += inbox.post(Some(value), |a, b| a.is_some() && b.is_some()) as usize;
        }
        wakeups += inbox.post(None, |_, _| false) as usize;
        assert_eq!(wakeups, 1);
        assert_eq!(inbox.take(), VecDeque::from([Some(99_999), None]));
        assert!(inbox.post(Some(100_000), |_, _| false));
        assert_eq!(inbox.take(), VecDeque::from([Some(100_000)]));
    }
}
