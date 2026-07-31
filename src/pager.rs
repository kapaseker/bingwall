use std::time::{Duration, Instant};

const PAGE_BATCH: usize = 10;
const TRANSITION_DURATION: Duration = Duration::from_millis(360);
const MIN_TRANSITION_DURATION: Duration = Duration::from_millis(120);
const SNAP_DISTANCE_RATIO: f32 = 0.12;
const SNAP_VELOCITY: f32 = 650.0;
const WHEEL_DEBOUNCE: Duration = Duration::from_millis(240);

/// Describes one normalized horizontal snap between pages.
#[derive(Debug, Clone)]
pub(crate) struct Transition {
    pub(crate) from: usize,
    pub(crate) to: usize,
    start_offset: f32,
    end_offset: f32,
    started: Instant,
    duration: Duration,
}

/// Tracks one active pointer drag and its filtered release velocity.
#[derive(Debug, Clone)]
struct Drag {
    start_x: f32,
    last_x: f32,
    last_at: Instant,
    velocity: f32,
}

/// Owns selection, metadata batching, and horizontal pager motion.
#[derive(Debug)]
pub(crate) struct Pager {
    item_count: usize,
    selected: usize,
    visible_count: usize,
    transition: Option<Transition>,
    offset: f32,
    drag: Option<Drag>,
    pointer_x: Option<f32>,
    width: f32,
    last_wheel: Option<Instant>,
}

impl Pager {
    /// Creates a pager positioned at the first available item.
    pub(crate) fn new(item_count: usize) -> Self {
        Self {
            item_count,
            selected: 0,
            visible_count: item_count.min(PAGE_BATCH),
            transition: None,
            offset: 0.0,
            drag: None,
            pointer_x: None,
            width: 1.0,
            last_wheel: None,
        }
    }

    /// Replaces the item collection and returns the pager to a safe first-page state.
    pub(crate) fn reset(&mut self, item_count: usize) {
        self.item_count = item_count;
        self.selected = 0;
        self.visible_count = item_count.min(PAGE_BATCH);
        self.transition = None;
        self.offset = 0.0;
        self.drag = None;
        self.pointer_x = None;
    }

    /// Returns the Selected Wallpaper index.
    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    /// Returns the count of Wallpaper Entry metadata currently exposed.
    #[cfg(test)]
    pub(crate) fn visible_count(&self) -> usize {
        self.visible_count
    }

    /// Moves to an adjacent item when the pager is idle and the destination exists.
    pub(crate) fn navigate(&mut self, direction: isize, now: Instant) -> bool {
        self.navigate_from_offset(direction, 0.0, now)
    }

    /// Moves to an adjacent item and starts a snap from a normalized offset.
    fn navigate_from_offset(&mut self, direction: isize, start_offset: f32, now: Instant) -> bool {
        if self.item_count == 0 || self.transition.is_some() || self.drag.is_some() {
            return false;
        }
        let next = self.selected.saturating_add_signed(direction);
        if next >= self.item_count || next == self.selected {
            return false;
        }
        let previous = self.selected;
        self.selected = next;
        if self.selected + 2 >= self.visible_count && self.visible_count < self.item_count {
            self.visible_count = (self.visible_count + PAGE_BATCH).min(self.item_count);
        }
        self.transition = Some(Transition {
            from: previous,
            to: next,
            start_offset,
            end_offset: -(direction.signum() as f32),
            started: now,
            duration: snap_duration(start_offset, -(direction.signum() as f32)),
        });
        self.offset = start_offset;
        true
    }

    /// Completes an elapsed pager transition.
    pub(crate) fn tick(&mut self, now: Instant) {
        if self
            .transition
            .as_ref()
            .is_some_and(|transition| now.duration_since(transition.started) >= transition.duration)
        {
            self.transition = None;
            self.offset = 0.0;
        }
    }

    /// Returns the normalized horizontal translation at a point in time.
    pub(crate) fn offset_at(&self, now: Instant) -> f32 {
        self.transition
            .as_ref()
            .map(|transition| {
                let progress = (now.duration_since(transition.started).as_secs_f32()
                    / transition.duration.as_secs_f32())
                .clamp(0.0, 1.0);
                transition.start_offset
                    + (transition.end_offset - transition.start_offset) * progress
            })
            .unwrap_or(self.offset)
    }

    /// Returns the active transition used to choose the two visible pages.
    pub(crate) fn transition(&self) -> Option<&Transition> {
        self.transition.as_ref()
    }

    /// Reports whether a transition or live drag currently owns pager motion.
    pub(crate) fn is_moving(&self) -> bool {
        self.transition.is_some() || self.drag.is_some()
    }

    /// Reports whether animation ticks are required.
    pub(crate) fn is_animating(&self) -> bool {
        self.transition.is_some()
    }

    /// Updates the pointer position and any active drag.
    pub(crate) fn pointer_moved(&mut self, x: f32, now: Instant) {
        self.pointer_x = Some(x);
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let elapsed = now.duration_since(drag.last_at).as_secs_f32();
        if elapsed > f32::EPSILON {
            let instantaneous = (x - drag.last_x) / elapsed;
            drag.velocity = drag.velocity * 0.65 + instantaneous * 0.35;
        }
        drag.last_x = x;
        drag.last_at = now;

        let mut offset = (x - drag.start_x) / self.width.max(1.0);
        if self.selected == 0 {
            offset = offset.min(0.0);
        }
        if self.selected + 1 >= self.item_count {
            offset = offset.max(0.0);
        }
        self.offset = offset.clamp(-1.0, 1.0);
    }

    /// Starts a pointer drag using the latest position or the viewport center.
    pub(crate) fn press(&mut self, width: f32, now: Instant) {
        if self.item_count == 0 || self.transition.is_some() {
            return;
        }
        self.width = width.max(1.0);
        let x = self.pointer_x.unwrap_or(self.width / 2.0);
        self.offset = 0.0;
        self.drag = Some(Drag {
            start_x: x,
            last_x: x,
            last_at: now,
            velocity: 0.0,
        });
    }

    /// Finishes a pointer drag by snapping to a neighbor or back to the current page.
    pub(crate) fn release(&mut self, now: Instant) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        let direction = snap_direction(self.offset, drag.velocity);
        if direction != 0 {
            return self.navigate_from_offset(direction, self.offset, now);
        }
        if self.offset.abs() > f32::EPSILON {
            self.transition = Some(Transition {
                from: self.selected,
                to: self.selected,
                start_offset: self.offset,
                end_offset: 0.0,
                started: now,
                duration: snap_duration(self.offset, 0.0),
            });
        }
        false
    }

    /// Cancels a lost pointer drag and restores the selected page offset.
    pub(crate) fn cancel_drag(&mut self) {
        self.drag = None;
        self.offset = 0.0;
    }

    /// Applies a debounced wheel navigation request.
    pub(crate) fn wheel(&mut self, direction: isize, now: Instant) -> bool {
        if self
            .last_wheel
            .is_some_and(|last| now.duration_since(last) < WHEEL_DEBOUNCE)
        {
            return false;
        }
        self.last_wheel = Some(now);
        self.navigate(direction, now)
    }
}

/// Chooses a snap direction from normalized distance and release velocity.
fn snap_direction(offset: f32, velocity: f32) -> isize {
    if offset <= -SNAP_DISTANCE_RATIO || velocity <= -SNAP_VELOCITY {
        1
    } else if offset >= SNAP_DISTANCE_RATIO || velocity >= SNAP_VELOCITY {
        -1
    } else {
        0
    }
}

/// Scales snap time by remaining distance while retaining visible minimum motion.
fn snap_duration(start_offset: f32, end_offset: f32) -> Duration {
    let distance = (end_offset - start_offset).abs().min(1.0);
    let millis = (TRANSITION_DURATION.as_millis() as f32 * distance)
        .round()
        .max(MIN_TRANSITION_DURATION.as_millis() as f32);
    Duration::from_millis(millis as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verifies navigation stays in bounds and expands metadata in ten-entry batches.
    fn navigation_is_bounded_and_batched() {
        let mut pager = Pager::new(25);
        let started = Instant::now();

        assert!(!pager.navigate(-1, started));
        for step in 0..8 {
            let now = started + Duration::from_secs(step);
            assert!(pager.navigate(1, now));
            pager.tick(now + Duration::from_secs(1));
        }

        assert_eq!(pager.selected(), 8);
        assert_eq!(pager.visible_count(), 20);
    }

    #[test]
    /// Verifies left and right transitions are linear mirror images with deliberate timing.
    fn transitions_are_linear_mirrors() {
        let started = Instant::now();
        let mut right = Pager::new(2);
        assert!(right.navigate(1, started));
        assert_eq!(right.offset_at(started + Duration::from_millis(180)), -0.5);

        right.tick(started + Duration::from_secs(1));
        assert!(right.navigate(-1, started + Duration::from_secs(2)));
        assert_eq!(right.offset_at(started + Duration::from_millis(2180)), 0.5);
    }

    #[test]
    /// Verifies a pointer drag follows the cursor and snaps to the adjacent page.
    fn pointer_drag_tracks_and_snaps() {
        let started = Instant::now();
        let mut pager = Pager::new(3);

        pager.pointer_moved(500.0, started);
        pager.press(1000.0, started);
        pager.pointer_moved(300.0, started + Duration::from_millis(200));
        assert_eq!(pager.offset_at(started + Duration::from_millis(200)), -0.2);

        assert!(pager.release(started + Duration::from_millis(210)));
        assert_eq!(pager.selected(), 1);
        let transition = pager.transition().expect("release starts a snap");
        assert_eq!((transition.from, transition.to), (0, 1));
        assert_eq!(pager.offset_at(started + Duration::from_millis(210)), -0.2);
    }

    #[test]
    /// Verifies either drag distance or release velocity can select the adjacent page.
    fn drag_snap_uses_distance_or_velocity() {
        let started = Instant::now();
        let mut distance = Pager::new(2);
        distance.pointer_moved(500.0, started);
        distance.press(1000.0, started);
        distance.pointer_moved(380.0, started + Duration::from_millis(200));
        assert!(distance.release(started + Duration::from_millis(210)));

        let mut velocity = Pager::new(2);
        velocity.pointer_moved(500.0, started);
        velocity.press(1000.0, started);
        velocity.pointer_moved(490.0, started + Duration::from_millis(1));
        assert!(velocity.release(started + Duration::from_millis(2)));

        let mut slow = Pager::new(2);
        slow.pointer_moved(500.0, started);
        slow.press(1000.0, started);
        slow.pointer_moved(450.0, started + Duration::from_millis(200));
        assert!(!slow.release(started + Duration::from_millis(210)));
        assert_eq!(slow.selected(), 0);
    }

    #[test]
    /// Verifies wheel navigation is debounced and replacing the feed resets selection.
    fn wheel_is_debounced_and_feed_reset_is_safe() {
        let started = Instant::now();
        let mut pager = Pager::new(1);

        assert!(!pager.wheel(1, started));
        pager.reset(3);
        assert!(!pager.wheel(1, started + Duration::from_millis(100)));
        assert_eq!(pager.selected(), 0);
        assert!(pager.wheel(1, started + Duration::from_millis(250)));

        pager.reset(1);
        assert_eq!(pager.selected(), 0);
        assert_eq!(pager.visible_count(), 1);
        assert!(!pager.is_moving());
    }
}
