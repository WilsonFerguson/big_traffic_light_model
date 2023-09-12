use piston_window::*;

use crate::{
    traffic_light_controller::{self, SimplifiedCar, TrafficLightController},
    HEIGHT, WIDTH,
};

pub const MAX_SPEED: f64 = 5.0;
pub const ACCELERATION: f64 = 0.15;
const DECELERATION: f64 = 0.3;

const DISTANCE_THRESHOLD: f64 = 5.0;

pub const CAR_WIDTH: f64 = 50.0; // 75.0, 50
const CAR_HEIGHT: f64 = 33.0; // 50.0, 33
const ARROW_STROKE_WEIGHT: f64 = 2.5; //  5.0, 2.5

pub const LANE_WIDTH: f64 = CAR_HEIGHT * 2.0;

pub const NUM_PATH_POINTS: usize = 25; // Higher = more accurate path but more expensive

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Origin {
    North,
    South,
    East,
    West,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Direction {
    Left,
    Right,
    Straight,
}

impl Direction {
    pub fn from(i: usize) -> Direction {
        match i {
            0 => Direction::Left,
            1 => Direction::Right,
            2 => Direction::Straight,
            _ => panic!("Invalid direction"),
        }
    }
}

#[derive(Clone)]
pub struct Car {
    pub id: usize,
    pub origin: Origin,
    direction: Direction,
    position: (f64, f64),
    rotation: f64,
    target_rotation: f64,
    speed: f64,
    stopped: bool,
    automatically_stopped: bool,
    path: Vec<(f64, f64)>,
    path_index: usize,
    path_index_on_red_change: Option<usize>,
    path_index_at_intersection: usize,
    pub finished: bool,
    through_intersection: bool,
}

impl Car {
    pub fn new(id: usize, origin: Origin, direction: Direction) -> Car {
        let rotation: f64 = match origin {
            Origin::North => 90.0,
            Origin::South => 270.0,
            Origin::East => 180.0,
            Origin::West => 0.0,
        };
        let path: Vec<(f64, f64)> = match direction {
            Direction::Left => generate_left_turn_path(origin),
            Direction::Right => generate_right_turn_path(origin),
            Direction::Straight => generate_straight_path(origin),
        };
        Car {
            id,
            origin,
            direction,
            position: get_position(origin, direction),
            rotation,
            target_rotation: rotation,
            speed: 0.0,
            stopped: false,
            automatically_stopped: false,
            path,
            path_index: 1,
            path_index_on_red_change: None,
            path_index_at_intersection: NUM_PATH_POINTS / 3
                + if direction == Direction::Straight {
                    1
                } else {
                    0
                },
            finished: false,
            through_intersection: false,
        }
    }

    fn get_distance_to_closest_car(&mut self, cars: &Vec<Car>) -> f64 {
        let mut closest_distance = f64::MAX;

        cars.clone()
            .iter()
            .filter(|c| c.origin == self.origin && c.direction == self.direction && c.id != self.id)
            .for_each(|c| {
                let (x, y) = self.position;
                let (cx, cy) = c.position;
                match self.origin {
                    Origin::North => {
                        if cy < y {
                            return;
                        }
                    }
                    Origin::South => {
                        if cy > y {
                            return;
                        }
                    }
                    Origin::East => {
                        if cx > x {
                            return;
                        }
                    }
                    Origin::West => {
                        if cx < x {
                            return;
                        }
                    }
                }
                let distance = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                closest_distance = closest_distance.min(distance);
            });

        closest_distance
    }

    fn automatically_stop(&mut self, cars: &Vec<Car>) {
        if self.through_intersection {
            return;
        }

        let closest_distance = self.get_distance_to_closest_car(cars);
        // Make sure cars that are on top of each other don't stop
        if !self.stopped && closest_distance < CAR_WIDTH * 2.0 && closest_distance > 3.0 {
            self.stopped = true;
            self.automatically_stopped = true;
        } else if self.stopped && self.automatically_stopped && closest_distance > CAR_WIDTH * 2.0 {
            self.stopped = false;
            self.automatically_stopped = false;
        }
    }

    fn stop_for_traffic_light(&mut self, traffic_light: &mut TrafficLightController) {
        if self.through_intersection {
            self.stopped = false;
            return;
        }

        let mut can_go = traffic_light.is_green(self.origin, self.direction);
        // If it's red but I'm not at the intersection, I can keep going
        if !can_go && self.path_index != self.path_index_at_intersection {
            can_go = true;
        }

        // If it's green, reset path index on red change
        if can_go {
            self.path_index_on_red_change = None;
        }

        self.stopped = !can_go;
    }

    pub fn update(&mut self, cars: &Vec<Car>, traffic_light: &mut TrafficLightController) {
        // If we have entered the intersection, remove ourselves from the traffic light
        if !self.through_intersection && self.past_intersection() {
            self.through_intersection = true;
            traffic_light.remove_car(SimplifiedCar::new(self.origin, self.direction));
        }
        // If it's yellow and I'm right at the intersection, remove myself from the traffic light
        // (to update clearance times)
        if traffic_light.is_yellow(self.origin, self.direction)
            && self.path_index == self.path_index_at_intersection
            && !self.through_intersection
        {
            traffic_light.remove_car(SimplifiedCar::new(self.origin, self.direction));
            self.through_intersection = true;
        }

        self.stop_for_traffic_light(traffic_light);
        self.automatically_stop(cars);

        if !self.stopped {
            self.speed += ACCELERATION;
            if self.speed > MAX_SPEED {
                self.speed = MAX_SPEED;
            }
        } else {
            if self.speed > 0.0 {
                self.speed -= DECELERATION;
            } else {
                self.speed = 0.0;
            }
        }

        // Move towards next point in path
        let dx = self.rotation.to_radians().cos() * self.speed;
        let dy = self.rotation.to_radians().sin() * self.speed;
        self.position.0 += dx;
        self.position.1 += dy;

        if self.intersects_point(self.path[self.path_index]) {
            self.path_index += 1;
            if self.path_index >= self.path.len() {
                self.path_index = 0;
                self.finished = true;
            }

            if self.path_index >= 1 {
                let dx = self.path[self.path_index].0 - self.position.0;
                let dy = self.path[self.path_index].1 - self.position.1;
                self.target_rotation = dy.atan2(dx).to_degrees();
            }
        }

        // Rotate towards target rotation
        let mut diff = self.target_rotation - self.rotation;
        if diff > 180.0 {
            diff -= 360.0;
        } else if diff < -180.0 {
            diff += 360.0;
        }
        self.rotation += diff * 0.5;

        // self.draw(cars, context, graphics);
    }

    fn past_intersection(&self) -> bool {
        self.path_index > self.path_index_at_intersection
    }

    fn intersects_point(&self, point: (f64, f64)) -> bool {
        let dx = self.position.0 - point.0;
        let dy = self.position.1 - point.1;
        let distance = (dx * dx + dy * dy).sqrt();
        distance < DISTANCE_THRESHOLD
    }

    pub fn intersects_rect(&self, other_vertices: [(f64, f64); 4]) -> bool {
        let my_vertices = self.vertices();
        let my_lines = my_vertices
            .iter()
            .zip(my_vertices.iter().cycle().skip(1))
            .map(|(&a, &b)| (a, b))
            .collect::<Vec<_>>();
        let other_lines = other_vertices
            .iter()
            .zip(other_vertices.iter().cycle().skip(1))
            .map(|(&a, &b)| (a, b))
            .collect::<Vec<_>>();

        my_lines.iter().any(|&line| {
            for other_line in &other_lines {
                if line_intersect(line, &other_line) {
                    return true;
                }
            }
            return false;
        })
    }

    fn intersects_rect_with_two_cars(
        vertices: [(f64, f64); 4],
        other_vertices: [(f64, f64); 4],
    ) -> bool {
        let my_lines = vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .map(|(&a, &b)| (a, b))
            .collect::<Vec<_>>();
        let other_lines = other_vertices
            .iter()
            .zip(other_vertices.iter().cycle().skip(1))
            .map(|(&a, &b)| (a, b))
            .collect::<Vec<_>>();

        my_lines.iter().any(|&line| {
            for other_line in &other_lines {
                if line_intersect(line, &other_line) {
                    return true;
                }
            }
            return false;
        })
    }

    pub fn cars_intersect(
        position1: (f64, f64),
        rotation1: f64,
        position2: (f64, f64),
        rotation2: f64,
    ) -> bool {
        let vertices1 = Car::vertices_with_pos_and_rot(position1, rotation1);
        let vertices2 = Car::vertices_with_pos_and_rot(position2, rotation2);
        Car::intersects_rect_with_two_cars(vertices1, vertices2)
    }

    fn get_vertex(&self, vertex: (f64, f64)) -> (f64, f64) {
        (
            self.position.0 + (vertex.0 * self.rotation.to_radians().cos())
                - (vertex.1 * self.rotation.to_radians().sin()),
            self.position.1
                + (vertex.0 * self.rotation.to_radians().sin())
                + (vertex.1 * self.rotation.to_radians().cos()),
        )
    }

    fn get_vertex_with_pos_and_rot(
        vertex: (f64, f64),
        position: (f64, f64),
        rotation: f64,
    ) -> (f64, f64) {
        (
            position.0 + (vertex.0 * rotation.to_radians().cos())
                - (vertex.1 * rotation.to_radians().sin()),
            position.1
                + (vertex.0 * rotation.to_radians().sin())
                + (vertex.1 * rotation.to_radians().cos()),
        )
    }

    pub fn vertices(&self) -> [(f64, f64); 4] {
        let half_width = CAR_WIDTH / 2.0;
        let half_height = CAR_HEIGHT / 2.0;

        let front_left = (-half_width, -half_height);
        let front_right = (half_width, -half_height);
        let back_left = (-half_width, half_height);
        let back_right = (half_width, half_height);

        [
            self.get_vertex(front_left),
            self.get_vertex(front_right),
            self.get_vertex(back_right),
            self.get_vertex(back_left),
        ]
    }

    fn vertices_with_pos_and_rot(position: (f64, f64), rotation: f64) -> [(f64, f64); 4] {
        let half_width = CAR_WIDTH / 2.0;
        let half_height = CAR_HEIGHT / 2.0;

        let front_left = (-half_width, -half_height);
        let front_right = (half_width, -half_height);
        let back_left = (-half_width, half_height);
        let back_right = (half_width, half_height);

        [
            Car::get_vertex_with_pos_and_rot(front_left, position, rotation),
            Car::get_vertex_with_pos_and_rot(front_right, position, rotation),
            Car::get_vertex_with_pos_and_rot(back_right, position, rotation),
            Car::get_vertex_with_pos_and_rot(back_left, position, rotation),
        ]
    }

    pub fn draw(&self, cars: &Vec<Car>, context: &Context, graphics: &mut G2d) {
        let alpha = 1.0;
        let transform = context
            .transform
            .trans(self.position.0, self.position.1)
            .rot_deg(self.rotation);

        let fill_color = if cars
            .iter()
            .filter(|c| c.id != self.id)
            .any(|c| self.intersects_rect(c.vertices()))
        {
            [1.0, 0.0, 0.0, alpha]
        } else {
            [1.0, 1.0, 1.0, alpha]
        };
        rectangle_from_to(
            fill_color,
            [-CAR_WIDTH / 2.0, -CAR_HEIGHT / 2.0],
            [CAR_WIDTH / 2.0, CAR_HEIGHT / 2.0],
            transform,
            graphics,
        );

        match self.direction {
            Direction::Straight => Line::new_round([0.0, 0.0, 0.0, alpha], ARROW_STROKE_WEIGHT)
                .draw_arrow(
                    [-CAR_WIDTH / 2.5, 0.0, CAR_WIDTH / 2.5, 0.0],
                    CAR_HEIGHT / 2.5,
                    &DrawState::default(),
                    transform,
                    graphics,
                ),
            Direction::Left => Line::new_round([0.0, 0.0, 0.0, alpha], ARROW_STROKE_WEIGHT)
                .draw_arrow(
                    [0.0, CAR_HEIGHT / 2.5, 0.0, -CAR_HEIGHT / 2.5],
                    CAR_HEIGHT / 2.5,
                    &DrawState::default(),
                    transform,
                    graphics,
                ),
            Direction::Right => Line::new_round([0.0, 0.0, 0.0, alpha], ARROW_STROKE_WEIGHT)
                .draw_arrow(
                    [0.0, -CAR_HEIGHT / 2.5, 0.0, CAR_HEIGHT / 2.5],
                    CAR_HEIGHT / 2.5,
                    &DrawState::default(),
                    transform,
                    graphics,
                ),
        }

        // self.draw_path(context, graphics);
    }

    fn draw_path(&self, context: &Context, graphics: &mut G2d) {
        self.path.iter().for_each(|&point| {
            line_from_to(
                [1.0, 0.0, 0.0, 1.0],
                3.0,
                [point.0 - 2.0, point.1 - 2.0],
                [point.0 + 2.0, point.1 + 2.0],
                context.transform,
                graphics,
            );
        });
    }

    pub fn calculate_waiting_point_index(car: &traffic_light_controller::SimplifiedCar) -> usize {
        NUM_PATH_POINTS / 3
            + if car.direction == Direction::Straight {
                1
            } else {
                0
            }
    }

    pub fn calculate_path(car: &traffic_light_controller::SimplifiedCar) -> Vec<(f64, f64)> {
        match car.direction {
            Direction::Left => generate_left_turn_path(car.origin),
            Direction::Right => generate_right_turn_path(car.origin),
            Direction::Straight => generate_straight_path(car.origin),
        }
    }
}

fn get_position(origin: Origin, direction: Direction) -> (f64, f64) {
    let middle = (WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
    let offset = match direction {
        Direction::Left => 0.0,
        Direction::Straight => 1.0,
        Direction::Right => 2.0,
    };
    match origin {
        Origin::North => (
            middle.0 - LANE_WIDTH / 2.0 - offset * LANE_WIDTH,
            -CAR_WIDTH / 2.0,
        ),
        Origin::South => (
            middle.0 + LANE_WIDTH / 2.0 + offset * LANE_WIDTH,
            HEIGHT as f64 + CAR_WIDTH / 2.0,
        ),
        Origin::East => (
            WIDTH as f64 + CAR_WIDTH / 2.0,
            middle.1 - LANE_WIDTH / 2.0 - offset * LANE_WIDTH,
        ),
        Origin::West => (
            -CAR_WIDTH / 2.0,
            middle.1 + LANE_WIDTH / 2.0 + offset * LANE_WIDTH,
        ),
    }
}

/// Generates the initial straight that all cars have to do before they can turn
fn generate_straight_path_third(origin: Origin, direction: Direction) -> Vec<(f64, f64)> {
    let vertical_point_gap = (HEIGHT as f64 / 2.0 - LANE_WIDTH * 3.0 + CAR_WIDTH / 2.0) as f64
        / (NUM_PATH_POINTS / 3) as f64;
    let horizontal_point_gap = (WIDTH as f64 / 2.0 - LANE_WIDTH * 3.0 + CAR_WIDTH / 2.0) as f64
        / (NUM_PATH_POINTS / 3) as f64;
    let position = get_position(origin, direction);

    match origin {
        Origin::North => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0, position.1 + i as f64 * vertical_point_gap))
            .collect(),
        Origin::South => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0, position.1 - (i as f64 * vertical_point_gap)))
            .collect(),
        Origin::East => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0 - (i as f64 * horizontal_point_gap), position.1))
            .collect(),
        Origin::West => (0..NUM_PATH_POINTS / 3)
            .map(|i| (position.0 + (i as f64 * horizontal_point_gap), position.1))
            .collect(),
    }
}

fn generate_left_turn_path(origin: Origin) -> Vec<(f64, f64)> {
    let middle = (WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
    // Initial straight
    let mut path = generate_straight_path_third(origin, Direction::Left);

    // Turn
    let turn_origin = match origin {
        Origin::North => (
            middle.0 + LANE_WIDTH as f64 * 3.0,
            middle.1 - LANE_WIDTH as f64 * 3.0,
        ),
        Origin::South => (
            middle.0 - LANE_WIDTH as f64 * 3.0,
            middle.1 + LANE_WIDTH as f64 * 3.0,
        ),
        Origin::East => (
            middle.0 + LANE_WIDTH as f64 * 3.0,
            middle.1 + LANE_WIDTH as f64 * 3.0,
        ),
        Origin::West => (
            middle.0 - LANE_WIDTH as f64 * 3.0,
            middle.1 - LANE_WIDTH as f64 * 3.0,
        ),
    };
    let turn_path = match origin {
        Origin::North => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    - std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.cos() * LANE_WIDTH * 3.5,
                    turn_origin.1 - angle.sin() * LANE_WIDTH * 3.5,
                )
            })
            .collect::<Vec<_>>(),
        Origin::South => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    + std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.cos() * LANE_WIDTH * 3.5,
                    turn_origin.1 - angle.sin() * LANE_WIDTH * 3.5,
                )
            })
            .collect::<Vec<_>>(),
        Origin::East => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    + std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.sin() * LANE_WIDTH * 3.5,
                    turn_origin.1 + angle.cos() * LANE_WIDTH * 3.5,
                )
            })
            .collect::<Vec<_>>(),
        Origin::West => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle = (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI
                    / 2.0
                    + std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 + angle.sin() * LANE_WIDTH * 3.5,
                    turn_origin.1 - angle.cos() * LANE_WIDTH * 3.5,
                )
            })
            .collect::<Vec<_>>(),
    };
    path.extend(turn_path.iter().rev().collect::<Vec<_>>());

    // Direction::Left so that the turning car goes into the nearest lane
    let mut last_third_path = generate_straight_path_third(
        match origin {
            Origin::North => Origin::West,
            Origin::South => Origin::East,
            Origin::East => Origin::North,
            Origin::West => Origin::South,
        },
        Direction::Left,
    );
    last_third_path.iter_mut().for_each(|point| match origin {
        Origin::North => point.0 += middle.0 + LANE_WIDTH * 4.0,
        Origin::South => point.0 -= middle.0 + LANE_WIDTH * 4.0,
        Origin::East => point.1 += middle.1 + LANE_WIDTH * 4.0,
        Origin::West => point.1 -= middle.1 + LANE_WIDTH * 4.0,
    });

    path.extend(last_third_path);
    path
}

fn generate_right_turn_path(origin: Origin) -> Vec<(f64, f64)> {
    let middle = (WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
    // Initial straight
    let mut path = generate_straight_path_third(origin, Direction::Right);

    // Turn
    let turn_origin = match origin {
        Origin::North => (middle.0 - LANE_WIDTH * 3.0, middle.1 - LANE_WIDTH * 3.0),
        Origin::South => (middle.0 + LANE_WIDTH * 3.0, middle.1 + LANE_WIDTH * 3.0),
        Origin::East => (middle.0 + LANE_WIDTH * 3.0, middle.1 - LANE_WIDTH * 3.0),
        Origin::West => (middle.0 - LANE_WIDTH * 3.0, middle.1 + LANE_WIDTH * 3.0),
    };
    let turn_path = match origin {
        Origin::North => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 + angle.cos() * LANE_WIDTH / 2.0,
                    turn_origin.1 + angle.sin() * LANE_WIDTH / 2.0,
                )
            })
            .collect::<Vec<_>>(),
        Origin::South => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.cos() * LANE_WIDTH / 2.0,
                    turn_origin.1 - angle.sin() * LANE_WIDTH / 2.0,
                )
            })
            .collect::<Vec<_>>(),
        Origin::East => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 - angle.sin() * LANE_WIDTH / 2.0,
                    turn_origin.1 + angle.cos() * LANE_WIDTH / 2.0,
                )
            })
            .collect::<Vec<_>>(),
        Origin::West => (0..NUM_PATH_POINTS / 3)
            .map(|i| {
                let angle =
                    (i as f64) / (NUM_PATH_POINTS as f64 / 3.0) * std::f64::consts::PI / 2.0;
                (
                    turn_origin.0 + angle.sin() * LANE_WIDTH / 2.0,
                    turn_origin.1 - angle.cos() * LANE_WIDTH / 2.0,
                )
            })
            .collect::<Vec<_>>(),
    };
    path.extend(turn_path);

    // Direction::Right so that the turning car goes into the nearest lane
    let mut last_third_path = generate_straight_path_third(
        match origin {
            Origin::North => Origin::East,
            Origin::South => Origin::West,
            Origin::East => Origin::South,
            Origin::West => Origin::North,
        },
        Direction::Right,
    );
    last_third_path.iter_mut().for_each(|point| match origin {
        Origin::North => point.0 -= middle.0 + LANE_WIDTH * 4.0,
        Origin::South => point.0 += middle.0 + LANE_WIDTH * 4.0,
        Origin::East => point.1 -= middle.1 + LANE_WIDTH * 4.0,
        Origin::West => point.1 += middle.1 + LANE_WIDTH * 4.0,
    });

    path.extend(last_third_path);
    path
}

fn generate_straight_path(origin: Origin) -> Vec<(f64, f64)> {
    let vertical_point_gap = (HEIGHT as f64 + CAR_WIDTH / 2.0) as f64 / NUM_PATH_POINTS as f64;
    let horizontal_point_gap = (WIDTH as f64 + CAR_WIDTH / 2.0) as f64 / NUM_PATH_POINTS as f64;

    let position = get_position(origin, Direction::Straight);
    match origin {
        Origin::North => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0, position.1 + (i as f64 * vertical_point_gap)));
            }
            path
        }
        Origin::South => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0, position.1 - (i as f64 * vertical_point_gap)));
            }
            path
        }
        Origin::East => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0 - (i as f64 * horizontal_point_gap), position.1));
            }
            path
        }
        Origin::West => {
            let mut path = Vec::new();
            for i in 0..NUM_PATH_POINTS {
                path.push((position.0 + (i as f64 * horizontal_point_gap), position.1));
            }
            path
        }
    }
}

fn ccw(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    (c.1 - a.1) * (b.0 - a.0) > (b.1 - a.1) * (c.0 - a.0)
}

fn line_intersect(line: ((f64, f64), (f64, f64)), other_line: &((f64, f64), (f64, f64))) -> bool {
    let a = line.0;
    let b = line.1;
    let c = other_line.0;
    let d = other_line.1;
    ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d)
}
