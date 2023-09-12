extern crate find_folder;
extern crate piston_window;
use car::LANE_WIDTH;
use piston_window::*;
use rand::Rng;
use std::{
    path,
    time::{self, Duration, Instant},
};

mod car;
mod traffic_light;
mod traffic_light_controller;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 1280;

pub const USE_ENTRY_TIME: bool = true;
/// Allow cars to go into the intersection when they have a yellow light
pub const ALLOW_GO_ON_YELLOW: bool = true;

pub const YELLOW_TIME: Duration = Duration::from_millis(1500);
pub const MINIMUM_GREEN_TIME: Duration = Duration::from_millis(200);

fn draw_map(context: &Context, graphics: &mut G2d) {
    let middle = (WIDTH as f64 / 2.0, HEIGHT as f64 / 2.0);
    [
        [0.0, 0.0],
        [middle.0 + LANE_WIDTH * 3.0, 0.0],
        [0.0, middle.1 + LANE_WIDTH * 3.0],
        [middle.0 + LANE_WIDTH * 3.0, middle.1 + LANE_WIDTH * 3.0],
    ]
    .iter()
    .for_each(|&start| {
        rectangle(
            [0.0, 1.0, 0.0, 1.0],
            [
                start[0],
                start[1],
                WIDTH as f64 / 2.0 - LANE_WIDTH * 3.0,
                HEIGHT as f64 / 2.0 - LANE_WIDTH * 3.0,
            ],
            context.transform,
            graphics,
        );
    });

    let dash_gap_percent = 4.0 / 5.0;
    let num_dashes: u32 = 8;
    let dash_width = 2.0;

    // Horizontal dashes
    let dash_length =
        (middle.0 - LANE_WIDTH * 3.0) / (num_dashes as f64 * (1.0 + dash_gap_percent));
    let dash_gap = dash_length * dash_gap_percent;
    for i in 0..(((middle.0 - LANE_WIDTH * 3.0) / (dash_length + dash_gap)) as u32) {
        let mut start = i as f64 * (dash_length + dash_gap) + dash_gap / 2.0;
        for _ in 0..2 {
            for j in -2..=2 {
                if j == 0 {
                    continue;
                }

                let y = middle.1 + LANE_WIDTH * j as f64;
                line_from_to(
                    [1.0; 4],
                    dash_width,
                    [start, y],
                    [start + dash_length, y],
                    context.transform,
                    graphics,
                );
            }
            start += middle.0 + LANE_WIDTH * 3.0;
        }
    }

    // Vertical dashes
    let dash_length =
        (middle.1 - LANE_WIDTH * 3.0) / (num_dashes as f64 * (1.0 + dash_gap_percent));
    let dash_gap = dash_length * dash_gap_percent;
    for i in 0..(((middle.1 - LANE_WIDTH * 3.0) / (dash_length + dash_gap)) as u32) {
        let mut start = i as f64 * (dash_length + dash_gap) + dash_gap / 2.0;
        for _ in 0..2 {
            for j in -2..=2 {
                if j == 0 {
                    continue;
                }
                let x = middle.0 + LANE_WIDTH * j as f64;
                line_from_to(
                    [1.0; 4],
                    dash_width,
                    [x, start],
                    [x, start + dash_length],
                    context.transform,
                    graphics,
                );
            }
            start += middle.1 + LANE_WIDTH * 3.0;
        }
    }

    // Solid lines
    for i in 0..2 {
        line_from_to(
            [1.0; 4],
            dash_width,
            [i as f64 * (middle.0 + LANE_WIDTH * 3.0), middle.1],
            [
                i as f64 * (middle.0 + LANE_WIDTH * 3.0) + middle.0 - LANE_WIDTH * 3.0,
                middle.1,
            ],
            context.transform,
            graphics,
        );
        line_from_to(
            [1.0; 4],
            dash_width,
            [middle.0, i as f64 * (middle.1 + LANE_WIDTH * 3.0)],
            [
                middle.0,
                i as f64 * (middle.1 + LANE_WIDTH * 3.0) + middle.0 - LANE_WIDTH * 3.0,
            ],
            context.transform,
            graphics,
        );
    }
}

fn main() {
    let mut window: PistonWindow =
        WindowSettings::new("Insersection Traffic Manager", [WIDTH, HEIGHT])
            .exit_on_esc(true)
            .resizable(false)
            .build()
            .unwrap();

    let assets: path::PathBuf = find_folder::Search::ParentsThenKids(3, 3)
        .for_folder("assets")
        .unwrap();
    let mut glyphs: Glyphs = window.load_font(assets.join("Consolas.ttf")).unwrap();

    let mut cars: Vec<car::Car> = Vec::new();
    let mut id: usize = 0;

    let mut spawn_start = Instant::now();
    let mut spawn_increment = time::Duration::from_millis(650);
    let origins = [
        car::Origin::North,
        car::Origin::South,
        car::Origin::East,
        car::Origin::West,
    ];
    let mut origin_index = 0;

    let mut traffic_light = traffic_light_controller::TrafficLightController::new();

    let mut paused: bool = false;
    let mut last_paused: Instant = Instant::now();

    window.set_max_fps(60);
    while let Some(event) = window.next() {
        window.draw_2d(&event, |context, graphics, device| {
            clear([0.1; 4], graphics);

            draw_map(&context, graphics);

            let cars_clone = cars.clone();

            cars.iter_mut()
                .for_each(|car| car.draw(&cars_clone, &context, graphics));

            traffic_light.draw(&context, graphics);

            text::Text::new_color([0.0, 0.0, 0.0, 1.0], 20)
                .draw(
                    format!("Spawn increment: {:?}", spawn_increment).as_str(),
                    &mut glyphs,
                    &context.draw_state,
                    context.transform.trans(20.0, 35.0),
                    graphics,
                )
                .unwrap();
            glyphs.factory.encoder.flush(device);
        });

        if let Some(args) = event.update_args() {
            if !paused {
                // To fix waiting times not be consistent at lower frame rates, if the dt is ever
                // different than the target dt, we can add/subtract from all of the delays
                let target_dt = 1.0 / 120.0;
                let dt = args.dt;
                // When the actual dt is greater than the target dt, we need to add to the delays
                traffic_light.fix_delays(dt, target_dt);

                let cars_clone = cars.clone();
                traffic_light.update();

                if spawn_start.elapsed() >= spawn_increment {
                    let minimum_time = 300.0;
                    spawn_increment = time::Duration::from_millis(
                        (spawn_increment.as_millis() as f64 * 0.997).max(minimum_time) as u64,
                    );

                    let mut origin = origins[rand::thread_rng().gen_range(0..origins.len())];
                    if spawn_increment.as_millis() <= 600 {
                        origin = origins[origin_index];
                        origin_index = (origin_index + 1) % origins.len();
                    }
                    let direction = car::Direction::from(rand::thread_rng().gen_range(0..=2));
                    cars.push(car::Car::new(id, origin, direction));
                    traffic_light.add_car(traffic_light_controller::SimplifiedCar::new(
                        origin, direction,
                    ));
                    id += 1;
                    if id > 1000 {
                        id = 0;
                    }

                    spawn_start = time::Instant::now();
                }

                cars.iter_mut().for_each(|car| {
                    car.update(&cars_clone, &mut traffic_light);
                });

                for i in (0..cars.len()).rev() {
                    if cars[i].finished {
                        cars.remove(i);
                    }
                }
            }
        }
        event.button(|button| {
            if button.state != ButtonState::Press {
                return;
            }
            if let Button::Keyboard(key) = button.button {
                match key {
                    Key::Space => {
                        if paused {
                            paused = false;
                            traffic_light.unpause(last_paused.elapsed());
                            spawn_start += last_paused.elapsed();
                        } else {
                            last_paused = Instant::now();
                            paused = true;
                        }
                    }
                    _ => (),
                }
            };
        });
    }
}
