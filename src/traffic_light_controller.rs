use piston_window::*;
use std::{collections::HashMap, time::Duration};

use crate::{
    car::{self},
    traffic_light::{TrafficLight, TrafficLightState},
    ALLOW_GO_ON_YELLOW,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimplifiedCar {
    pub origin: car::Origin,
    pub direction: car::Direction,
}

impl SimplifiedCar {
    pub fn new(origin: car::Origin, direction: car::Direction) -> SimplifiedCar {
        SimplifiedCar { origin, direction }
    }
}

pub struct TrafficLightController {
    queue: HashMap<SimplifiedCar, usize>,
    traffic_lights: Vec<TrafficLight>,
}

impl TrafficLightController {
    pub fn new() -> TrafficLightController {
        TrafficLightController {
            queue: TrafficLightController::generate_queue(),
            traffic_lights: TrafficLightController::generate_traffic_lights(),
        }
    }

    pub fn fix_delays(&mut self, dt: f64, target_dt: f64) {
        let delay = target_dt - dt;
        for traffic_light in &mut self.traffic_lights {
            traffic_light.fix_delay(delay);
        }
    }

    pub fn update(&mut self) {
        let queue_lengths: Vec<usize> = self
            .traffic_lights
            .iter()
            .map(|traffic_light| self.queue(traffic_light.origin, traffic_light.direction))
            .collect();
        for (i, traffic_light) in self.traffic_lights.iter_mut().enumerate() {
            traffic_light.update(queue_lengths[i]);
        }

        // If a traffic light is waiting, see if its queue is greater than combined queue of all of
        // the lights that are currently green that it would have to cross. If so, make those
        // light's red and make this light green.

        // List of lights that are allowed to become green
        // (traffic light index, queue length, red clearance time)
        let mut lights_to_make_green: Vec<(usize, usize, Duration)> = Vec::new();
        for i in 0..self.traffic_lights.len() {
            if queue_lengths[i] == 0 {
                continue;
            }

            let mut total_queue_length = 0;
            let mut max_delay: Duration = Duration::from_millis(0);
            let mut can_change = true;
            let intersecting_lights = &self.traffic_lights[i].intersecting_lights;
            for light in intersecting_lights.keys() {
                // If that light cannot be changed to green yet, then continue to next light
                if !self.get_traffic_light(light.0, light.1).can_change_to_red() {
                    can_change = false;
                    break;
                }

                // Add to the queue if that light is green
                if self.get_traffic_light(light.0, light.1).state != TrafficLightState::Red {
                    total_queue_length += self.queue(light.0, light.1);
                    if intersecting_lights.get(light).unwrap() > &max_delay {
                        max_delay = *intersecting_lights.get(light).unwrap();
                    }
                }
            }
            if queue_lengths[i] <= total_queue_length || !can_change {
                // if !can_change {
                continue;
            }

            // Change the light to green
            // self.traffic_lights[i].change_to_green(max_delay);
            lights_to_make_green.push((i, queue_lengths[i], max_delay));
        }

        // After doing this, some lights that want to change green would conflict with each other
        // Find the best combination of lights that allows for the most cars to go through. Meaning
        // that the total queue length is maximized
        let mut i = 0;
        loop {
            if i >= lights_to_make_green.len() {
                break;
            }

            let mut light_to_remove: Option<usize> = None;
            let intersecting_lights =
                &self.traffic_lights[lights_to_make_green[i].0].intersecting_lights;
            // Go through each line in intersecting lights and see if it conflicts with any other
            // light in the lights_to_make_green vector
            for (other_index, (light, queue_length, max_delay)) in
                lights_to_make_green.iter().enumerate()
            {
                // If the light is not in the intersecting lights, then it can't conflict
                if !intersecting_lights.contains_key(&(
                    self.traffic_lights[*light].origin,
                    self.traffic_lights[*light].direction,
                )) {
                    continue;
                }

                // Whoever has a lower queue length gets removed from the lights_to_make_green
                if queue_lengths[lights_to_make_green[i].0] < *queue_length {
                    light_to_remove = Some(i);
                    break;
                } else if queue_lengths[lights_to_make_green[i].0] > *queue_length {
                    light_to_remove = Some(other_index);
                    break;
                } else {
                    // Whowever has a longer delay gets removed from the lights_to_make_green
                    if lights_to_make_green[i].2 > *max_delay {
                        light_to_remove = Some(i);
                        break;
                    } else {
                        light_to_remove = Some(other_index);
                        break;
                    }
                }
            }

            // If there is a light to remove, remove it
            if let Some(light_to_remove) = light_to_remove {
                lights_to_make_green.remove(light_to_remove);
                i = 0;
            } else {
                i += 1;
            }
        }

        for light in lights_to_make_green {
            self.traffic_lights[light.0].change_to_green(light.2);
        }
    }

    fn queue(&self, origin: car::Origin, direction: car::Direction) -> usize {
        *self
            .queue
            .get(&SimplifiedCar::new(origin, direction))
            .unwrap()
    }

    pub fn draw(&self, context: &Context, graphics: &mut G2d) {
        for traffic_light in &self.traffic_lights {
            traffic_light.draw(context, graphics);
        }
    }

    pub fn add_car(&mut self, car: SimplifiedCar) {
        if let Some(queue) = self.queue.get_mut(&car) {
            *queue += 1;
        }
    }

    pub fn remove_car(&mut self, car: SimplifiedCar) {
        if let Some(queue) = self.queue.get_mut(&car) {
            *queue -= 1;
        }
    }

    /// Returns if the light is green or yellow for the given origin.
    pub fn is_green(&self, origin: car::Origin, direction: car::Direction) -> bool {
        self.get_traffic_light(origin, direction).state != TrafficLightState::Red
    }

    pub fn is_yellow(&self, origin: car::Origin, direction: car::Direction) -> bool {
        ALLOW_GO_ON_YELLOW
            && self.get_traffic_light(origin, direction).state == TrafficLightState::Yellow
    }

    pub fn generate_traffic_lights() -> Vec<TrafficLight> {
        let mut traffic_lights = Vec::new();
        for origin in vec![
            car::Origin::North,
            car::Origin::South,
            car::Origin::East,
            car::Origin::West,
        ] {
            for direction in vec![
                car::Direction::Left,
                car::Direction::Straight,
                car::Direction::Right,
            ] {
                traffic_lights.push(TrafficLight::new(origin, direction));
            }
        }
        traffic_lights
    }

    pub fn generate_queue() -> HashMap<SimplifiedCar, usize> {
        let mut queue = HashMap::new();
        for origin in vec![
            car::Origin::North,
            car::Origin::South,
            car::Origin::East,
            car::Origin::West,
        ] {
            for direction in vec![
                car::Direction::Left,
                car::Direction::Straight,
                car::Direction::Right,
            ] {
                queue.insert(SimplifiedCar::new(origin, direction), 0);
            }
        }
        queue
    }

    pub fn get_traffic_light(
        &self,
        origin: car::Origin,
        direction: car::Direction,
    ) -> &TrafficLight {
        // Using the above code, the traffic lights are generated the order:
        // NorthLeft, NorthStraight, NorthRight, SouthLeft, SouthStraight, SouthRight, EastLeft, EastStraight, EastRight, WestLeft, WestStraight, WestRight
        let origin_index = match origin {
            car::Origin::North => 0,
            car::Origin::South => 3,
            car::Origin::East => 6,
            car::Origin::West => 9,
        };
        let direction_index = match direction {
            car::Direction::Left => 0,
            car::Direction::Straight => 1,
            car::Direction::Right => 2,
        };
        &self.traffic_lights[origin_index + direction_index]
    }

    pub fn unpause(&mut self, time_elapsed: Duration) {
        for traffic_light in &mut self.traffic_lights {
            traffic_light.unpause(time_elapsed);
        }
    }
}
