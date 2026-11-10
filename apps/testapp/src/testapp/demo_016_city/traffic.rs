//! Cars driving along the streets: lanes, traffic lights and keeping distance.

/// The direction a street runs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// What a traffic light shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Green,
    Yellow,
    Red,
}

/// Seconds of one traffic light cycle: X streets green, yellow, all red, then the same for Y.
pub const CYCLE: f32 = 20.0;
const GREEN: f32 = 7.5;
const YELLOW: f32 = 1.5;

/// What the lights of streets along `axis` show at `time`.
pub fn signal(axis: Axis, time: f32) -> Signal {
    let t = time.rem_euclid(CYCLE);
    let t = match axis {
        Axis::X => t,
        Axis::Y => (t - CYCLE / 2.0).rem_euclid(CYCLE),
    };
    if t < GREEN {
        Signal::Green
    } else if t < GREEN + YELLOW {
        Signal::Yellow
    } else {
        Signal::Red
    }
}

/// A lane: a straight line along a street, driven one way, wrapping around at its ends.
pub struct Lane {
    pub axis: Axis,
    /// The other coordinate of the lane's centre line.
    pub offset: f32,
    /// +1 or -1: the way cars go along the axis.
    pub direction: f32,
    /// From the start (-direction * length / 2) along the lane: where cars must stop at red.
    pub stop_lines: Vec<f32>,
}

/// The length of every lane; cars leave at one end and come back at the other.
pub const LANE_LENGTH: f32 = 360.0;
pub const CAR_LENGTH: f32 = 4.6;
/// Space kept to the car ahead when stopped.
const GAP: f32 = 2.5;
const ACCELERATION: f32 = 2.5;
const BRAKING: f32 = 4.0;
const MAX_BRAKING: f32 = 9.0;

impl Lane {
    /// The lane along `axis` at `offset`, driven in `direction`, with intersections whose
    /// stop lines are `stop_back` before the crossing streets' centres at `crossings`.
    pub fn new(axis: Axis, offset: f32, direction: f32, crossings: &[f32], stop_back: f32) -> Self {
        let mut stop_lines: Vec<f32> = crossings
            .iter()
            .map(|&c| (c * direction + LANE_LENGTH / 2.0 - stop_back).rem_euclid(LANE_LENGTH))
            .collect();
        stop_lines.sort_by(f32::total_cmp);
        Self {
            axis,
            offset,
            direction,
            stop_lines,
        }
    }

    /// The world (x, y) of `distance` along the lane, and the heading (radians).
    pub fn place(&self, distance: f32) -> (f32, f32, f32) {
        let along = (distance - LANE_LENGTH / 2.0) * self.direction;
        match self.axis {
            Axis::X => (
                along,
                self.offset,
                if self.direction > 0.0 {
                    0.0
                } else {
                    std::f32::consts::PI
                },
            ),
            Axis::Y => (
                self.offset,
                along,
                if self.direction > 0.0 {
                    std::f32::consts::FRAC_PI_2
                } else {
                    -std::f32::consts::FRAC_PI_2
                },
            ),
        }
    }

    /// How far ahead of `distance` the next stop line is.
    fn next_stop(&self, distance: f32) -> Option<f32> {
        self.stop_lines
            .iter()
            .map(|&line| (line - distance).rem_euclid(LANE_LENGTH))
            .min_by(f32::total_cmp)
    }
}

/// A car on a lane.
pub struct Car {
    pub lane: usize,
    /// Front bumper, along the lane.
    pub distance: f32,
    pub speed: f32,
    pub cruise: f32,
}

/// All the cars.
pub struct Traffic {
    pub lanes: Vec<Lane>,
    pub cars: Vec<Car>,
}

impl Traffic {
    /// Moves every car by `dt` seconds at `time`: towards its cruising speed, braking for the
    /// car ahead and for lights that aren't green (unless too close to stop at yellow).
    pub fn step(&mut self, time: f32, dt: f32) {
        // How far each car's way is free: to the car ahead in its lane, and to a red light.
        let mut free = vec![f32::MAX; self.cars.len()];
        for (lane_index, lane) in self.lanes.iter().enumerate() {
            let mut cars: Vec<usize> = (0..self.cars.len())
                .filter(|&c| self.cars[c].lane == lane_index)
                .collect();
            cars.sort_by(|&a, &b| self.cars[a].distance.total_cmp(&self.cars[b].distance));
            for (k, &car) in cars.iter().enumerate() {
                if cars.len() > 1 {
                    let ahead = cars[(k + 1) % cars.len()];
                    let gap = (self.cars[ahead].distance - self.cars[car].distance)
                        .rem_euclid(LANE_LENGTH);
                    free[car] = free[car].min(gap - CAR_LENGTH - GAP);
                }
                let state = signal(lane.axis, time);
                if let (Some(stop), true) = (
                    lane.next_stop(self.cars[car].distance),
                    state != Signal::Green,
                ) {
                    let speed = self.cars[car].speed;
                    let can_stop = stop > speed * speed / (2.0 * BRAKING) + 0.5;
                    if state == Signal::Red || can_stop {
                        free[car] = free[car].min(stop - 0.3);
                    }
                }
            }
        }
        for (car, free) in self.cars.iter_mut().zip(free) {
            let safe = (2.0 * BRAKING * free.max(0.0)).sqrt();
            let target = car.cruise.min(safe);
            let change = (target - car.speed).clamp(-MAX_BRAKING * dt, ACCELERATION * dt);
            car.speed = (car.speed + change).max(0.0);
            car.distance = (car.distance + car.speed * dt).rem_euclid(LANE_LENGTH);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lights_take_turns() {
        for step in 0..200 {
            let t = step as f32 * 0.1;
            let (x, y) = (signal(Axis::X, t), signal(Axis::Y, t));
            assert!(
                x == Signal::Red || y == Signal::Red,
                "{t}: both {x:?} and {y:?}"
            );
        }
        assert_eq!(signal(Axis::X, 1.0), Signal::Green);
        assert_eq!(signal(Axis::Y, 1.0), Signal::Red);
        assert_eq!(signal(Axis::Y, 11.0), Signal::Green);
    }

    #[test]
    fn lanes_place_cars_along_their_streets() {
        let lane = Lane::new(Axis::Y, 3.0, -1.0, &[0.0], 5.0);
        let (x, y, heading) = lane.place(LANE_LENGTH / 2.0 + 10.0);
        assert_eq!((x, y), (3.0, -10.0));
        assert!((heading + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        // Driving -Y, the stop line before the crossing at y = 0 is at y = 5.
        let (_, y, _) = lane.place(lane.stop_lines[0]);
        assert!((y - 5.0).abs() < 1e-4, "{y}");
    }

    #[test]
    fn cars_stop_at_red_and_keep_their_distance() {
        let lane = Lane::new(Axis::X, 0.0, 1.0, &[0.0], 5.0);
        let stop = lane.stop_lines[0];
        let mut traffic = Traffic {
            lanes: vec![lane],
            cars: vec![
                Car {
                    lane: 0,
                    distance: stop - 60.0,
                    speed: 10.0,
                    cruise: 12.0,
                },
                Car {
                    lane: 0,
                    distance: stop - 80.0,
                    speed: 10.0,
                    cruise: 12.0,
                },
            ],
        };
        // X is red from 9 s to 19 s: run during it.
        let mut time = 9.0;
        while time < 18.5 {
            traffic.step(time, 1.0 / 60.0);
            time += 1.0 / 60.0;
            let gap = traffic.cars[0].distance - traffic.cars[1].distance;
            assert!(gap > CAR_LENGTH + 0.5, "cars collided at {time}: {gap}");
        }
        let first = &traffic.cars[0];
        assert!(
            first.speed < 0.1 && first.distance <= stop && first.distance > stop - 2.0,
            "{} {}",
            first.distance,
            first.speed
        );
        // Once green, they go.
        while time < 25.0 {
            traffic.step(time, 1.0 / 60.0);
            time += 1.0 / 60.0;
        }
        assert!(traffic.cars[0].distance > stop + 5.0 || traffic.cars[0].distance < 1.0);
    }
}
