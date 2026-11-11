//! Animations: changes spread over time, driven by an `Animator` or by a `Scene`.
//!
//! An animation changes a context `C`, which it gets on every update: usually the `Scene`
//! (scene animations, see `Scene::add_animation`), which includes the scene's cameras, or
//! anything else the app animates with its own `Animator<C>`, e.g. the app's own state. Closures capture what they change by value: node handles,
//! start and end values; never references.
//!
//! - `SlideAnimation` runs for a set time and calls its step with the progress, 0 to 1, then
//!   its `on_finish` callback;
//! - `PerpetualAnimation` calls its step every update with the time elapsed, until it returns
//!   `ControlFlow::Break`.
//!
//! ```ignore
//! // Slide a node up by 2 over half a second, slowing down at the end, then remove it.
//! let start = scene.get_node(node).unwrap().get_transform();
//! scene.add_animation(
//!     None,
//!     SlideAnimation::new(0.5, move |scene: &mut Scene, p| {
//!         if let Some(n) = scene.get_node_mut(node) {
//!             n.set_transform(Mat4::from_translation(Vec3::new(0.0, 2.0 * p, 0.0)) * start);
//!         }
//!     })
//!     .easing(Easing::CubicOut)
//!     .on_finish(move |scene: &mut Scene| {
//!         let _ = scene.remove_node(node);
//!     })
//!     .target(node),
//! );
//! ```
//!
//! Animations are `Send + Sync`, so the scenes and animators holding them can be moved to, or
//! shared with, other threads: closures may capture handles, numbers or `Arc`s, but not `Rc`s or
//! `RefCell`s.

use std::ops::ControlFlow;

use crate::easing::Easing;

/// Something that changes a context `C` over time. See the module documentation.
pub trait Animation<C>: Send + Sync {
    /// Advances the animation by `dt` seconds and applies it to `ctx`. Returns `true` when the
    /// animation is finished; it is then not updated again.
    fn update(&mut self, ctx: &mut C, dt: f32) -> bool;

    /// Ends the animation now, as if its time had run out: applies its final state and runs its
    /// end-of-animation callback. Does nothing if it is already finished.
    fn finish(&mut self, ctx: &mut C);

    /// Sets the time elapsed since the animation started, in seconds. The default ignores it.
    fn set_elapsed(&mut self, _time: f32) {}

    /// Node this animation changes, if any. A scene cancels an animation when its target node
    /// is removed.
    fn get_target(&self) -> Option<usize> {
        None
    }
}

type SlideStep<C> = Box<dyn FnMut(&mut C, f32) + Send + Sync>;
type FinishCallback<C> = Box<dyn FnOnce(&mut C) + Send + Sync>;
type PerpetualStep<C> = Box<dyn FnMut(&mut C, f32, f32) -> ControlFlow<()> + Send + Sync>;

/// An animation lasting a set time. On every update it calls its step with the progress: the
/// share of the time elapsed, 0 to 1, passed through its easing. The last step gets exactly 1
/// (the easing's end); `on_finish` runs right after it.
pub struct SlideAnimation<C> {
    duration: f32,
    elapsed: f32,
    easing: Easing,
    step: SlideStep<C>,
    on_finish: Option<FinishCallback<C>>,
    target: Option<usize>,
    finished: bool,
}

impl<C> SlideAnimation<C> {
    /// An animation lasting `duration` seconds, calling `step(ctx, progress)` on every update.
    /// A `duration` of 0 or less finishes on the first update, with progress 1.
    pub fn new(duration: f32, step: impl FnMut(&mut C, f32) + Send + Sync + 'static) -> Self {
        Self {
            duration,
            elapsed: 0.0,
            easing: Easing::Linear,
            step: Box::new(step),
            on_finish: None,
            target: None,
            finished: false,
        }
    }

    /// Sets the easing applied to the progress (default `Easing::Linear`).
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Sets a callback run once, right after the last step.
    pub fn on_finish(mut self, on_finish: impl FnOnce(&mut C) + Send + Sync + 'static) -> Self {
        self.on_finish = Some(Box::new(on_finish));
        self
    }

    /// Sets the node this animation changes (see `Animation::get_target`).
    pub fn target(mut self, node: usize) -> Self {
        self.target = Some(node);
        self
    }

    pub fn get_duration(&self) -> f32 {
        self.duration
    }

    pub fn get_elapsed(&self) -> f32 {
        self.elapsed
    }

    fn complete(&mut self, ctx: &mut C) {
        self.finished = true;
        self.elapsed = self.duration.max(0.0);
        (self.step)(ctx, self.easing.apply(1.0));
        if let Some(on_finish) = self.on_finish.take() {
            on_finish(ctx);
        }
    }
}

impl<C> Animation<C> for SlideAnimation<C> {
    fn update(&mut self, ctx: &mut C, dt: f32) -> bool {
        if self.finished {
            return true;
        }
        self.elapsed += dt.max(0.0);
        if self.elapsed >= self.duration {
            self.complete(ctx);
            return true;
        }
        let progress = self.easing.apply(self.elapsed / self.duration);
        (self.step)(ctx, progress);
        false
    }

    fn finish(&mut self, ctx: &mut C) {
        if !self.finished {
            self.complete(ctx);
        }
    }

    fn set_elapsed(&mut self, time: f32) {
        self.elapsed = time.max(0.0);
    }

    fn get_target(&self) -> Option<usize> {
        self.target
    }
}

/// An animation with no set end. On every update it calls its step with the update's `dt` and
/// the time elapsed since it started (including this update), both in seconds; it ends when the
/// step returns `ControlFlow::Break`.
pub struct PerpetualAnimation<C> {
    elapsed: f32,
    step: PerpetualStep<C>,
    target: Option<usize>,
    finished: bool,
}

impl<C> PerpetualAnimation<C> {
    /// An animation calling `step(ctx, dt, elapsed)` on every update.
    pub fn new(
        step: impl FnMut(&mut C, f32, f32) -> ControlFlow<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            elapsed: 0.0,
            step: Box::new(step),
            target: None,
            finished: false,
        }
    }

    /// Sets the node this animation changes (see `Animation::get_target`).
    pub fn target(mut self, node: usize) -> Self {
        self.target = Some(node);
        self
    }

    pub fn get_elapsed(&self) -> f32 {
        self.elapsed
    }
}

impl<C> Animation<C> for PerpetualAnimation<C> {
    fn update(&mut self, ctx: &mut C, dt: f32) -> bool {
        if self.finished {
            return true;
        }
        let dt = dt.max(0.0);
        self.elapsed += dt;
        self.finished = (self.step)(ctx, dt, self.elapsed).is_break();
        self.finished
    }

    /// Stops the animation; there is no final state to apply.
    fn finish(&mut self, _ctx: &mut C) {
        self.finished = true;
    }

    fn set_elapsed(&mut self, time: f32) {
        self.elapsed = time.max(0.0);
    }

    fn get_target(&self) -> Option<usize> {
        self.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context recording what animations did.
    #[derive(Default)]
    struct Log {
        steps: Vec<f32>,
        finishes: usize,
    }

    fn slide(duration: f32) -> SlideAnimation<Log> {
        SlideAnimation::new(duration, |log: &mut Log, p| log.steps.push(p))
            .on_finish(|log: &mut Log| log.finishes += 1)
    }

    fn assert_close(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-5, "{actual:?} vs {expected:?}");
        }
    }

    #[test]
    fn slide_reports_progress_and_finishes() {
        let mut log = Log::default();
        let mut a = slide(2.0);
        assert!(!a.update(&mut log, 0.5));
        assert!(!a.update(&mut log, 1.0));
        assert_eq!(log.finishes, 0);
        assert!(a.update(&mut log, 1.0));
        assert_close(&log.steps, &[0.25, 0.75, 1.0]);
        assert_eq!(log.finishes, 1);
        assert_eq!(a.get_elapsed(), 2.0);

        // Finished: nothing more happens.
        assert!(a.update(&mut log, 1.0));
        a.finish(&mut log);
        assert_eq!(log.steps.len(), 3);
        assert_eq!(log.finishes, 1);
    }

    #[test]
    fn slide_ends_exactly_on_its_duration() {
        let mut log = Log::default();
        let mut a = slide(1.0);
        assert!(a.update(&mut log, 1.0));
        assert_eq!(log.steps, vec![1.0]);
    }

    #[test]
    fn slide_applies_its_easing() {
        let mut log = Log::default();
        let mut a = slide(1.0).easing(Easing::QuadraticIn);
        a.update(&mut log, 0.5);
        a.update(&mut log, 0.6);
        assert_close(&log.steps, &[0.25, 1.0]);

        // Overshooting easings still end exactly at 1.
        let mut log = Log::default();
        let mut a = slide(1.0).easing(Easing::ElasticOut);
        a.update(&mut log, 2.0);
        assert_eq!(log.steps, vec![1.0]);
    }

    #[test]
    fn slide_without_duration_finishes_at_once() {
        for duration in [0.0, -1.0] {
            let mut log = Log::default();
            let mut a = slide(duration);
            assert!(a.update(&mut log, 0.0));
            assert_eq!(log.steps, vec![1.0]);
            assert_eq!(log.finishes, 1);
        }
    }

    #[test]
    fn finishing_a_slide_early_applies_its_end() {
        let mut log = Log::default();
        let mut a = slide(10.0);
        a.update(&mut log, 1.0);
        a.finish(&mut log);
        assert_close(&log.steps, &[0.1, 1.0]);
        assert_eq!(log.finishes, 1);
        assert!(a.update(&mut log, 1.0));
        assert_eq!(log.steps.len(), 2);
    }

    #[test]
    fn slide_elapsed_can_be_set() {
        let mut log = Log::default();
        let mut a = slide(4.0);
        a.update(&mut log, 1.0);
        a.set_elapsed(3.0);
        a.update(&mut log, 0.0);
        a.set_elapsed(-1.0);
        a.update(&mut log, 0.0);
        assert_close(&log.steps, &[0.25, 0.75, 0.0]);
    }

    #[test]
    fn negative_steps_do_not_go_back() {
        let mut log = Log::default();
        let mut a = slide(2.0);
        a.update(&mut log, 1.0);
        a.update(&mut log, -1.0);
        assert_close(&log.steps, &[0.5, 0.5]);
    }

    #[test]
    fn target_is_reported() {
        assert_eq!(slide(1.0).get_target(), None);
        assert_eq!(slide(1.0).target(7).get_target(), Some(7));
        let p = PerpetualAnimation::new(|_: &mut Log, _, _| ControlFlow::Continue(()));
        assert_eq!(p.get_target(), None);
        assert_eq!(p.target(3).get_target(), Some(3));
    }

    #[test]
    fn perpetual_sums_time_and_stops_on_break() {
        let mut log = Log::default();
        let mut a = PerpetualAnimation::new(|log: &mut Log, dt, elapsed| {
            log.steps.push(dt);
            log.steps.push(elapsed);
            if elapsed >= 1.0 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert!(!a.update(&mut log, 0.25));
        assert!(!a.update(&mut log, 0.5));
        assert!(a.update(&mut log, 0.5));
        assert_close(&log.steps, &[0.25, 0.25, 0.5, 0.75, 0.5, 1.25]);

        // Stopped: the step isn't called again.
        assert!(a.update(&mut log, 0.5));
        assert_eq!(log.steps.len(), 6);
    }

    #[test]
    fn perpetual_can_be_finished_and_reset() {
        let mut log = Log::default();
        let mut a = PerpetualAnimation::new(|log: &mut Log, _, elapsed| {
            log.steps.push(elapsed);
            ControlFlow::Continue(())
        });
        a.update(&mut log, 1.0);
        a.set_elapsed(10.0);
        a.update(&mut log, 1.0);
        assert_close(&log.steps, &[1.0, 11.0]);
        a.finish(&mut log);
        assert!(a.update(&mut log, 1.0));
        assert_eq!(log.steps.len(), 2);
    }

    #[test]
    fn animations_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SlideAnimation<Log>>();
        assert_send_sync::<PerpetualAnimation<Log>>();
        assert_send_sync::<Box<dyn Animation<Log>>>();
    }
}
