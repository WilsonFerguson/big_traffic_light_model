use std::collections::HashMap;
use std::f64::consts::PI;
use std::time::Duration;
use std::time::Instant;

use crate::car;
use crate::car::NUM_PATH_POINTS;
use crate::traffic_light_controller::SimplifiedCar;
use crate::HEIGHT;
use crate::LANE_WIDTH;
use crate::MINIMUM_GREEN_TIME;
use crate::USE_ENTRY_TIME;
use crate::WIDTH;
use crate::YELLOW_TIME;
use piston_window::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficLightState {
    Red,
    Yellow,
    Green,
}

#[derive(Debug)]
pub struct TrafficLight {
    pub origin: car::Origin,
    pub direction: car::Direction,
    pub state: TrafficLightState,
    /// HashMap that contains every other light that cars would intersect with. The values are the
    /// yellow + red times for each light.
    pub intersecting_lights: HashMap<(car::Origin, car::Direction), Duration>,
    pub green_start: Instant,
    pub red_start: Instant,
    /// Time when this light got the go ahead to change to green.
    change_to_green_start: Instant,
    change_to_green_delay: Duration,
    should_change_to_green: bool,
}

impl TrafficLight {
    pub fn new(origin: car::Origin, direction: car::Direction) -> TrafficLight {
        let mut intersecting_lights = HashMap::new();
        let waiting_car = SimplifiedCar::new(origin, direction);
        for other_origin in vec![
            car::Origin::North,
            car::Origin::South,
            car::Origin::East,
            car::Origin::West,
        ] {
            for other_direction in vec![
                car::Direction::Left,
                car::Direction::Straight,
                car::Direction::Right,
            ] {
                if other_origin == origin && other_direction == direction {
                    continue;
                }
                let moving_car = SimplifiedCar::new(other_origin, other_direction);
                let red_clearance_time = calculate_red_clearance_time(&moving_car, &waiting_car);
                if red_clearance_time.as_millis() > 0 {
                    intersecting_lights.insert((other_origin, other_direction), red_clearance_time);
                }
            }
        }
        TrafficLight {
            origin,
            direction,
            state: TrafficLightState::Red,
            intersecting_lights,
            green_start: Instant::now(),
            red_start: Instant::now(),
            change_to_green_start: Instant::now(),
            change_to_green_delay: Duration::from_millis(0),
            should_change_to_green: false,
        }
    }

    pub fn fix_delay(&mut self, delay_secs: f64) {
        // When positive delay, that means that the frame took less time than expected. This means that we need to subtract that time from the timers.
        // When the delay is negative, we need to add that time to the timers.
        if delay_secs > 0.0 {
            self.green_start -= Duration::from_secs_f64(delay_secs);
            self.red_start -= Duration::from_secs_f64(delay_secs);
            self.change_to_green_start -= Duration::from_secs_f64(delay_secs);
        } else {
            self.green_start += Duration::from_secs_f64(-delay_secs);
            self.red_start += Duration::from_secs_f64(-delay_secs);
            self.change_to_green_start += Duration::from_secs_f64(-delay_secs);
        }
    }

    pub fn change_to_red(&mut self) {
        self.red_start = Instant::now();
        self.state = TrafficLightState::Yellow;
    }

    /// Returns true if it's been more than the minimum green time and we aren't about to change to
    /// green
    pub fn can_change_to_red(&self) -> bool {
        self.green_start.elapsed() >= MINIMUM_GREEN_TIME && !self.should_change_to_green
    }

    pub fn change_to_green(&mut self, delay: Duration) {
        self.change_to_green_start = Instant::now();
        self.change_to_green_delay = delay;
        self.should_change_to_green = true;
    }

    pub fn update(&mut self, queue: usize) {
        // Change to green
        if self.should_change_to_green
            && self.change_to_green_start.elapsed() >= self.change_to_green_delay
        {
            self.state = TrafficLightState::Green;
            self.green_start = Instant::now();
            self.should_change_to_green = false;
        }

        // Green and no one is going
        if self.state == TrafficLightState::Green && self.can_change_to_red() && queue == 0 {
            self.change_to_red();
        }

        // Yellow
        if self.state == TrafficLightState::Yellow {
            if self.red_start.elapsed() > YELLOW_TIME {
                self.state = TrafficLightState::Red;
            }
        }
    }

    pub fn unpause(&mut self, time_elapsed: Duration) {
        self.green_start += time_elapsed;
        self.red_start += time_elapsed;
    }

    pub fn draw(&self, context: &Context, graphics: &mut G2d) {
        let light_radius = 10.0;
        let light_spacing = (2.0 / 3.0) * light_radius;

        let alpha = 0.7;
        let green = [0.24, 0.96, 0.21, alpha];
        let yellow = [0.92, 0.95, 0.13, alpha];
        let red = [0.96, 0.19, 0.19, alpha];
        let dark_green = [0.05, 0.22, 0.04, alpha];
        let dark_yellow = [0.3, 0.32, 0.04, alpha];
        let dark_red = [0.34, 0.06, 0.06, alpha];

        let mut final_position = match self.origin {
            car::Origin::North => (0.0, -LANE_WIDTH * 3.1),
            car::Origin::South => (0.0, LANE_WIDTH * 3.1),
            car::Origin::East => (LANE_WIDTH * 3.1, 0.0),
            car::Origin::West => (-LANE_WIDTH * 3.1, 0.0),
        };
        final_position.0 += WIDTH as f64 / 2.0;
        final_position.1 += HEIGHT as f64 / 2.0;

        let mut offset = match self.direction {
            car::Direction::Left => 0.0,
            car::Direction::Straight => LANE_WIDTH,
            car::Direction::Right => LANE_WIDTH * 2.0,
        };
        offset += light_radius;
        match self.origin {
            car::Origin::North => final_position.0 -= offset,
            car::Origin::South => final_position.0 += offset,
            car::Origin::East => final_position.1 -= offset,
            car::Origin::West => final_position.1 += offset,
        };

        let transform = context
            .transform
            .trans(final_position.0, final_position.1)
            .rot_rad(match self.origin {
                car::Origin::North => PI,
                car::Origin::East => 3.0 * PI / 2.0,
                car::Origin::South => 0.0,
                car::Origin::West => PI / 2.0,
            });
        Rectangle::new_round([0.0, 0.0, 0.0, alpha], light_radius * 2.5).draw(
            [
                0.0,
                0.0,
                light_radius * 5.0,
                (light_radius * 2.0 + light_spacing) * 3.0 + light_spacing * 2.0,
            ],
            &context.draw_state,
            transform,
            graphics,
        );
        Rectangle::new_round_border([1.0, 1.0, 1.0, alpha], light_radius * 2.5, 1.5).draw(
            [
                0.0,
                0.0,
                light_radius * 5.0,
                (light_radius * 2.0 + light_spacing) * 3.0 + light_spacing * 2.0,
            ],
            &context.draw_state,
            transform,
            graphics,
        );
        let final_red = if self.state == TrafficLightState::Red {
            red
        } else {
            dark_red
        };
        let final_yellow = if self.state == TrafficLightState::Yellow {
            yellow
        } else {
            dark_yellow
        };
        let final_green = if self.state == TrafficLightState::Green {
            green
        } else {
            dark_green
        };
        let colors = [final_red, final_yellow, final_green];
        for i in 0..3 {
            ellipse(
                colors[i],
                [
                    light_radius + light_radius * 0.5,
                    light_radius + (i as f64 * (light_radius * 2.0 + light_spacing)),
                    light_radius * 2.0,
                    light_radius * 2.0,
                ],
                transform,
                graphics,
            );
        }

        if self.direction != car::Direction::Straight {
            let x = light_radius + light_radius * 0.5;
            let mut y = light_radius + 3.0 * (light_radius * 2.0 + light_spacing)
                - light_radius * 2.0
                + light_spacing / 2.0;
            if self.state == TrafficLightState::Yellow {
                y -= (light_radius * 2.0 - light_spacing) * 2.0;
            }
            if self.state == TrafficLightState::Red {
                y -= (light_radius * 2.0 - light_spacing) * 4.0;
            }

            let first_x = if self.direction == car::Direction::Right {
                x + 5.0
            } else {
                x + light_radius * 2.0 - 5.0
            };
            let second_x = if self.direction == car::Direction::Right {
                x + light_radius * 2.0 - 5.0
            } else {
                x + 5.0
            };
            Line::new_round([0.8, 0.8, 0.8, alpha], 2.0).draw_arrow(
                [first_x, y, second_x, y],
                light_radius / 2.0,
                &DrawState::default(),
                transform,
                graphics,
            );
        }
    }
}

/// Calculates the entry time of a car into the intersection given the car already in the
/// intersection and the currently waiting car
fn calculate_entry_time(moving_car: &SimplifiedCar, waiting_car: &SimplifiedCar) -> Duration {
    let moving_car_path = car::Car::calculate_path(moving_car);
    let waiting_car_path = car::Car::calculate_path(waiting_car);

    let mut moving_path_index: usize = 0;
    let mut waiting_path_index: usize = 0;

    for (i, point) in moving_car_path.iter().enumerate() {
        for (j, other_point) in waiting_car_path.iter().enumerate() {
            if i == 0 || j == 0 {
                continue;
            }

            let rotation = (point.1 - moving_car_path[i - 1].1)
                .atan2(point.0 - moving_car_path[i - 1].0)
                .to_degrees();
            let other_rotation = (other_point.1 - waiting_car_path[j - 1].1)
                .atan2(other_point.0 - waiting_car_path[j - 1].0)
                .to_degrees();
            if !car::Car::cars_intersect(*point, rotation, *other_point, other_rotation) {
                continue;
            }

            moving_path_index = i;
            waiting_path_index = j;
            break;
        }
        if moving_path_index != 0 {
            break;
        }
    }

    // Don't intersect
    if moving_path_index == 0 {
        return Duration::from_secs(100);
    }

    let end_index = (waiting_path_index - 1)
        .min(waiting_car_path.len() - 1)
        .max(0);
    let distance_to_collision = (car::Car::calculate_waiting_point_index(waiting_car)..=end_index)
        .map(|i| i as f64)
        .reduce(|acc, i| {
            let distance = ((waiting_car_path[i as usize].0 - waiting_car_path[i as usize + 1].0)
                .powi(2)
                + (waiting_car_path[i as usize].1 - waiting_car_path[i as usize + 1].1).powi(2))
            .sqrt();
            acc + distance
        })
        .unwrap_or(0.0);

    // Function: d = (1/2)at^2 assuming initial velocity is 0
    // So: t = sqrt(2d/a)
    let num_frames = (2.0 * distance_to_collision / car::ACCELERATION).sqrt();

    let frame_duration = 1000.0 / 60.0;
    return Duration::from_millis((num_frames * frame_duration) as u64);
}

fn calculate_red_clearance_time(
    moving_car: &SimplifiedCar,
    waiting_car: &SimplifiedCar,
) -> Duration {
    let waiting_car_path = car::Car::calculate_path(waiting_car);

    // let straight_distance = LANE_WIDTH * 4.0;
    // let left_distance = std::f64::consts::PI * LANE_WIDTH * 3.0 / 2.0;
    // let right_distance = std::f64::consts::PI * LANE_WIDTH / 2.0;
    //
    // let distance_covered = match waiting_car.direction {
    //     car::Direction::Straight => straight_distance,
    //     car::Direction::Left => left_distance,
    //     car::Direction::Right => right_distance,
    // };
    let first_point = car::Car::calculate_waiting_point_index(waiting_car);
    let points = waiting_car_path
        .iter()
        .skip(first_point)
        .take(NUM_PATH_POINTS / 3)
        .collect::<Vec<_>>();
    let mut distance_covered = 0.0;
    for i in 0..(points.len() - 1) {
        let distance = (points[i].0 - points[i + 1].0).hypot(points[i].1 - points[i + 1].1);
        distance_covered += distance;
    }

    let speed = car::MAX_SPEED;
    let frame_duration = 1000.0 / 60.0;

    let mut clearance_time = distance_covered / speed * frame_duration; // Raw all red time

    // Subtract entry time
    if USE_ENTRY_TIME {
        clearance_time -= calculate_entry_time(moving_car, waiting_car).as_millis() as f64;
    }
    clearance_time += YELLOW_TIME.as_millis() as f64; // Add in yellow at the start

    clearance_time = clearance_time.max(0.0);
    return Duration::from_millis(clearance_time as u64);
}
