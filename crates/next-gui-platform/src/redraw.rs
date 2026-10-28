use std::time::{Duration, Instant};
use winit::event_loop::{ActiveEventLoop, ControlFlow};

#[derive(Default)]
pub(crate) struct RedrawDeadline(Option<Instant>);

impl RedrawDeadline {
    pub(crate) fn request(&mut self, delay: Duration) {
        self.request_at(Instant::now(), delay);
    }

    fn request_at(&mut self, now: Instant, delay: Duration) {
        if let Some(next) = now.checked_add(delay) {
            self.0 = Some(self.0.map_or(next, |current| current.min(next)));
        }
    }

    fn take_due(&mut self, now: Instant) -> bool {
        if self.0.is_some_and(|deadline| now >= deadline) {
            self.0 = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let due = self.take_due(Instant::now());
        if let Some(next) = self.0 {
            event_loop.set_control_flow(match event_loop.control_flow() {
                ControlFlow::Poll => ControlFlow::Poll,
                ControlFlow::WaitUntil(current) => ControlFlow::WaitUntil(current.min(next)),
                ControlFlow::Wait => ControlFlow::WaitUntil(next),
            });
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redraw_deadlines_coalesce_do_not_fire_early_and_return_to_idle() {
        let now = Instant::now();
        let mut deadline = RedrawDeadline::default();
        assert!(!deadline.take_due(now));
        deadline.request_at(now, Duration::from_millis(400));
        deadline.request_at(now, Duration::from_millis(900));
        assert_eq!(deadline.0, Some(now + Duration::from_millis(400)));
        deadline.request_at(now, Duration::from_millis(200));
        assert!(!deadline.take_due(now + Duration::from_millis(199)));
        assert!(deadline.take_due(now + Duration::from_millis(200)));
        assert_eq!(deadline.0, None);
        assert!(!deadline.take_due(now + Duration::from_secs(30)));
        deadline.request_at(now, Duration::MAX);
        assert_eq!(deadline.0, None);
        deadline.request_at(now, Duration::ZERO);
        assert!(deadline.take_due(now));
        deadline.request_at(now, Duration::from_millis(50));
        assert!(deadline.take_due(now + Duration::from_secs(30)));
        assert!(!deadline.take_due(now + Duration::from_secs(31)));
    }
}
