//! `Animator`: runs animations (see `animation`) on a context `C`.
//!
//! Scenes have their own animator for animations that change the scene (`Scene::add_animation`,
//! driven by the engine every frame). Use a standalone `Animator<C>` for anything else, e.g. a
//! camera, and drive it from the app:
//!
//! ```ignore
//! self.camera_animator.advance_to(&mut self.camera, time);
//! ```
//!
//! Animations are addressed by handles, like a scene's nodes. As with other handles, a handle can
//! be given to a new animation once its animation has finished or been cancelled: forget it
//! then.
//!
//! While an animation runs, it is taken out of its slot, so callbacks can change the context
//! freely. For scene animations the context is the scene itself, so callbacks can add, finish
//! and cancel animations; animations added during an update start in the next one.

use std::collections::HashSet;

use crate::animation::Animation;
use crate::objdb::ObjDB;
use crate::object::Object;

/// An animation in an `Animator`.
struct Entry<C> {
    name: Option<String>,
    handle: usize,
    /// Unique within the animator: tells this animation apart from a later one given the same
    /// handle.
    serial: u64,
    /// `Animation::get_target`, read when the animation is added.
    target: Option<usize>,
    /// `None` while the animation is running (taken out by `take`).
    animation: Option<Box<dyn Animation<C>>>,
}

impl<C> Object for Entry<C> {
    fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    fn set_handle(&mut self, handle: usize) {
        self.handle = handle;
    }

    fn get_handle(&self) -> usize {
        self.handle
    }
}

/// An animation taken out of its animator to be run (see `Animator::take`).
pub(crate) struct Taken<C> {
    handle: usize,
    serial: u64,
    pub(crate) animation: Box<dyn Animation<C>>,
}

/// Animations of a context `C`. See the module documentation.
pub struct Animator<C> {
    entries: ObjDB<Entry<C>>,
    next_serial: u64,
    /// The time last passed to `advance_to`.
    last_time: Option<f32>,
}

impl<C> Animator<C> {
    pub fn new() -> Self {
        Self {
            entries: ObjDB::new(),
            next_serial: 0,
            last_time: None,
        }
    }

    /// Adds an animation, which runs from the next update, and returns its handle. `name` lets
    /// `find` look it up.
    pub fn add(
        &mut self,
        name: Option<String>,
        animation: impl Animation<C> + 'static,
    ) -> usize {
        self.add_boxed(name, Box::new(animation))
    }

    /// `add` for an animation already boxed.
    pub fn add_boxed(&mut self, name: Option<String>, animation: Box<dyn Animation<C>>) -> usize {
        let serial = self.next_serial;
        self.next_serial += 1;
        self.entries.add(Entry {
            name,
            handle: usize::MAX,
            serial,
            target: animation.get_target(),
            animation: Some(animation),
        })
    }

    /// Handle of the running animation named `name` (the first added, if several are).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries.get_by_name(name).map(|e| e.handle)
    }

    /// Whether `handle` is a running animation (not finished or cancelled).
    pub fn is_running(&self, handle: usize) -> bool {
        self.entries.get(handle).is_some()
    }

    /// Number of running animations.
    pub fn len(&self) -> usize {
        self.entries.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().next().is_none()
    }

    /// Advances every animation by `dt` seconds, removing the ones that finish.
    pub fn update(&mut self, ctx: &mut C, dt: f32) {
        for (handle, serial) in self.running() {
            if let Some(mut taken) = self.take(handle, serial) {
                let finished = taken.animation.update(ctx, dt);
                self.restore(taken, finished);
            }
        }
    }

    /// Advances every animation to `time` (in seconds, on any clock that only goes forward):
    /// by the time since the previous call, or by 0 on the first call.
    pub fn advance_to(&mut self, ctx: &mut C, time: f32) {
        let dt = self.advance_time(time);
        self.update(ctx, dt);
    }

    /// Ends an animation now (`Animation::finish`) and removes it. Does nothing for handles
    /// that aren't running, and for the animation currently being updated.
    pub fn finish(&mut self, ctx: &mut C, handle: usize) {
        let Some(serial) = self.serial_of(handle) else {
            return;
        };
        if let Some(mut taken) = self.take(handle, serial) {
            taken.animation.finish(ctx);
            self.restore(taken, true);
        }
    }

    /// Removes an animation without running any of its callbacks. Does nothing for handles that
    /// aren't running.
    pub fn cancel(&mut self, handle: usize) {
        self.entries.remove(handle);
    }

    /// Removes every animation without running any callbacks.
    pub fn clear(&mut self) {
        for (handle, _) in self.running() {
            self.entries.remove(handle);
        }
    }

    /// Sets the time elapsed since an animation started (`Animation::set_elapsed`). Does nothing
    /// for handles that aren't running, and for the animation currently being updated.
    pub fn set_elapsed(&mut self, handle: usize, time: f32) {
        if let Some(animation) = self
            .entries
            .get_mut(handle)
            .and_then(|e| e.animation.as_mut())
        {
            animation.set_elapsed(time);
        }
    }

    /// Cancels every animation whose target is one of `nodes`.
    pub fn cancel_targeting(&mut self, nodes: &HashSet<usize>) {
        let doomed: Vec<usize> = self
            .entries
            .iter()
            .filter(|e| e.target.is_some_and(|t| nodes.contains(&t)))
            .map(|e| e.handle)
            .collect();
        for handle in doomed {
            self.entries.remove(handle);
        }
    }

    /// Handles and serial numbers of the running animations.
    pub(crate) fn running(&self) -> Vec<(usize, u64)> {
        self.entries.iter().map(|e| (e.handle, e.serial)).collect()
    }

    /// Serial number of the running animation `handle`.
    pub(crate) fn serial_of(&self, handle: usize) -> Option<u64> {
        self.entries.get(handle).map(|e| e.serial)
    }

    /// The `dt` for `advance_to(time)`, remembering `time`.
    pub(crate) fn advance_time(&mut self, time: f32) -> f32 {
        let dt = self.last_time.map_or(0.0, |last| (time - last).max(0.0));
        self.last_time = Some(time);
        dt
    }

    /// Takes the animation `handle` out of its slot to run it, if it is still the one with
    /// `serial` and isn't already taken out. The slot stays, so its handle isn't given to
    /// another animation meanwhile; `restore` puts the animation back or removes the slot.
    pub(crate) fn take(&mut self, handle: usize, serial: u64) -> Option<Taken<C>> {
        let entry = self.entries.get_mut(handle)?;
        if entry.serial != serial {
            return None;
        }
        let animation = entry.animation.take()?;
        Some(Taken {
            handle,
            serial,
            animation,
        })
    }

    /// Ends what `take` started: removes the animation if it is `finished`, or puts it back. If
    /// it was cancelled meanwhile (its slot is gone, or holds another animation), drops it.
    pub(crate) fn restore(&mut self, taken: Taken<C>, finished: bool) {
        let Some(entry) = self.entries.get_mut(taken.handle) else {
            return;
        };
        if entry.serial != taken.serial {
            return;
        }
        if finished {
            self.entries.remove(taken.handle);
        } else {
            entry.animation = Some(taken.animation);
        }
    }
}

impl<C> Default for Animator<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::ControlFlow;

    use super::*;
    use crate::animation::{PerpetualAnimation, SlideAnimation};

    #[derive(Default)]
    struct Log {
        events: Vec<String>,
    }

    fn slide(name: &'static str, duration: f32) -> SlideAnimation<Log> {
        SlideAnimation::new(duration, move |log: &mut Log, p| {
            log.events.push(format!("{name} {p}"))
        })
        .on_finish(move |log: &mut Log| log.events.push(format!("{name} done")))
    }

    fn take_events(log: &mut Log) -> Vec<String> {
        std::mem::take(&mut log.events)
    }

    #[test]
    fn update_drives_animations_and_removes_finished_ones() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        assert!(animator.is_empty());
        let a = animator.add(None, slide("a", 1.0));
        let b = animator.add(None, slide("b", 2.0));
        assert_eq!(animator.len(), 2);

        animator.update(&mut log, 0.5);
        assert_eq!(take_events(&mut log), ["a 0.5", "b 0.25"]);

        animator.update(&mut log, 0.5);
        assert_eq!(take_events(&mut log), ["a 1", "a done", "b 0.5"]);
        assert!(!animator.is_running(a));
        assert!(animator.is_running(b));
        assert_eq!(animator.len(), 1);

        animator.update(&mut log, 1.0);
        assert_eq!(take_events(&mut log), ["b 1", "b done"]);
        assert!(animator.is_empty());
    }

    #[test]
    fn finish_jumps_to_the_end() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        let a = animator.add(None, slide("a", 4.0));
        animator.update(&mut log, 1.0);
        animator.finish(&mut log, a);
        assert_eq!(take_events(&mut log), ["a 0.25", "a 1", "a done"]);
        assert!(!animator.is_running(a));

        // Nothing for handles that aren't running.
        animator.finish(&mut log, a);
        animator.finish(&mut log, 99);
        assert!(log.events.is_empty());
    }

    #[test]
    fn cancel_runs_nothing() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        let a = animator.add(None, slide("a", 1.0));
        animator.cancel(a);
        animator.cancel(99);
        animator.update(&mut log, 2.0);
        assert!(log.events.is_empty());
        assert!(!animator.is_running(a));
    }

    #[test]
    fn clear_cancels_everything() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        animator.add(None, slide("a", 1.0));
        animator.add(None, slide("b", 1.0));
        animator.clear();
        assert!(animator.is_empty());
        animator.update(&mut log, 2.0);
        assert!(log.events.is_empty());
    }

    #[test]
    fn advance_to_updates_by_the_time_since_the_last_call() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        animator.add(None, slide("a", 4.0));
        animator.advance_to(&mut log, 10.0);
        animator.advance_to(&mut log, 11.0);
        animator.advance_to(&mut log, 11.0);
        animator.advance_to(&mut log, 13.0);
        assert_eq!(take_events(&mut log), ["a 0", "a 0.25", "a 0.25", "a 0.75"]);
    }

    #[test]
    fn set_elapsed_moves_an_animation_in_time() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        let a = animator.add(None, slide("a", 4.0));
        animator.set_elapsed(a, 2.0);
        animator.update(&mut log, 1.0);
        assert_eq!(take_events(&mut log), ["a 0.75"]);
    }

    #[test]
    fn find_looks_up_running_animations_by_name() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        let a = animator.add(Some("move".into()), slide("a", 1.0));
        assert_eq!(animator.find("move"), Some(a));
        assert_eq!(animator.find("other"), None);
        animator.update(&mut log, 1.0);
        assert_eq!(animator.find("move"), None);
    }

    #[test]
    fn perpetual_animations_stop_themselves() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        let a = animator.add(
            None,
            PerpetualAnimation::new(|log: &mut Log, _, elapsed| {
                log.events.push(format!("{elapsed}"));
                if elapsed >= 2.0 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            }),
        );
        animator.update(&mut log, 1.0);
        assert!(animator.is_running(a));
        animator.update(&mut log, 1.0);
        assert!(!animator.is_running(a));
        assert_eq!(take_events(&mut log), ["1", "2"]);
    }

    #[test]
    fn cancel_targeting_removes_animations_of_those_nodes() {
        let mut animator: Animator<Log> = Animator::new();
        let a = animator.add(None, slide("a", 1.0).target(1));
        let b = animator.add(None, slide("b", 1.0).target(2));
        let c = animator.add(None, slide("c", 1.0));
        animator.cancel_targeting(&HashSet::from([1, 5]));
        assert!(!animator.is_running(a));
        assert!(animator.is_running(b));
        assert!(animator.is_running(c));
    }

    #[test]
    fn restore_drops_an_animation_cancelled_while_taken() {
        let mut log = Log::default();
        let mut animator = Animator::new();
        let a = animator.add(None, slide("a", 4.0));
        let (handle, serial) = animator.running()[0];
        let taken = animator.take(handle, serial).unwrap();
        // Taken out: can't be taken again, finished or moved in time.
        assert!(animator.take(handle, serial).is_none());
        animator.finish(&mut log, a);
        animator.set_elapsed(a, 1.0);
        assert!(animator.is_running(a));

        // Cancelled meanwhile, and the handle given to another animation.
        animator.cancel(a);
        let b = animator.add(None, slide("b", 4.0));
        assert_eq!(a, b);
        animator.restore(taken, false);
        animator.update(&mut log, 1.0);
        assert_eq!(take_events(&mut log), ["b 0.25"]);
        assert_eq!(animator.len(), 1);
    }

    #[test]
    fn animator_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Animator<Log>>();
    }
}
