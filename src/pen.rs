use anyhow::Result;
use evdev::EventType as EvdevEventType;
use evdev::{Device, InputEvent};
use log::info;
use std::thread::sleep;
use std::time::Duration;

use crate::device::DeviceModel;

// Output dimensions remain the same for both devices
const VIRTUAL_WIDTH: u32 = 768;
const VIRTUAL_HEIGHT: u32 = 1024;
const STROKE_PASS_OFFSET: f32 = 0.8;
const STROKE_STEP: f32 = 1.5;

pub struct Pen {
    device: Option<Device>,
    device_model: DeviceModel,
}

impl Pen {
    pub fn new(no_draw: bool) -> Self {
        let device_model = DeviceModel::detect();
        info!("Pen using device model: {}", device_model.name());

        let pen_input_device = match device_model {
            DeviceModel::Remarkable2 => "/dev/input/event1",
            DeviceModel::RemarkablePaperPro => "/dev/input/event2",
            DeviceModel::Unknown => "/dev/input/event1", // Default to RM2
        };

        let device = if no_draw { None } else { Some(Device::open(pen_input_device).unwrap()) };

        Self { device, device_model }
    }

    pub fn draw_line_screen(&mut self, p1: (i32, i32), p2: (i32, i32)) -> Result<()> {
        self.draw_line(self.virtual_to_input(p1), self.virtual_to_input(p2))
    }

    pub fn draw_line(&mut self, (x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> Result<()> {
        // trace!("Drawing from ({}, {}) to ({}, {})", x1, y1, x2, y2);

        // We know this is a straight line
        // So figure out the length
        // Then divide it into enough steps to only go 10 units or so
        // Start at x1, y1
        // And then for each step add the right amount to x and y

        let length = ((x2 as f32 - x1 as f32).powf(2.0) + (y2 as f32 - y1 as f32).powf(2.0)).sqrt();
        // 5.0 is the maximum distance between points
        // If this is too small
        let steps = (length / 5.0).ceil() as i32;
        let dx = (x2 - x1) / steps;
        let dy = (y2 - y1) / steps;
        // trace!(
        //     "Drawing from ({}, {}) to ({}, {}) in {} steps",
        //     x1, y1, x2, y2, steps
        // );

        self.pen_up()?;
        self.goto_xy((x1, y1))?;
        self.pen_down()?;

        for i in 0..steps {
            let x = x1 + dx * i;
            let y = y1 + dy * i;
            self.goto_xy((x, y))?;
            // trace!("Drawing to point at ({}, {})", x, y);
        }

        self.pen_up()?;

        Ok(())
    }

    pub fn draw_bitmap(&mut self, bitmap: &[Vec<bool>]) -> Result<()> {
        let mut is_pen_down = false;
        for (y, row) in bitmap.iter().enumerate() {
            if !row.iter().any(|&pixel| pixel) {
                continue;
            }
            for (x, &pixel) in row.iter().enumerate() {
                if pixel {
                    if !is_pen_down {
                        self.goto_xy_virtual((x as i32, y as i32))?;
                        self.pen_down()?;
                        is_pen_down = true;
                        sleep(Duration::from_millis(1));
                    }
                    self.goto_xy_virtual((x as i32, y as i32))?;
                    self.goto_xy_virtual((x as i32 + 1, y as i32))?;
                } else if is_pen_down {
                    self.pen_up()?;
                    is_pen_down = false;
                    sleep(Duration::from_millis(1));
                }
            }
            self.pen_up()?;
            is_pen_down = false;
            sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    /// Traces one continuous path with the pen held down, offsetting each extra pass to thicken it.
    pub fn draw_stroke(&mut self, path: &[(f32, f32)], passes: u32) -> Result<()> {
        let Some(&start) = path.first() else {
            return Ok(());
        };
        self.goto_xy_virtual_f(start)?;
        self.pen_down()?;
        sleep(Duration::from_millis(1));
        let mut previous = start;
        for pass in 0..passes {
            let offset = pass as f32 * STROKE_PASS_OFFSET;
            let forward = pass % 2 == 0;
            let points: Vec<(f32, f32)> = if forward { path.to_vec() } else { path.iter().rev().copied().collect() };
            for (x, y) in points {
                let target = (x + offset, y);
                let distance = ((target.0 - previous.0).powi(2) + (target.1 - previous.1).powi(2)).sqrt();
                let steps = (distance / STROKE_STEP).ceil().max(1.0) as i32;
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    self.goto_xy_virtual_f((previous.0 + (target.0 - previous.0) * t, previous.1 + (target.1 - previous.1) * t))?;
                }
                previous = target;
            }
        }
        self.pen_up()?;
        sleep(Duration::from_millis(1));
        Ok(())
    }

    // fn draw_dot(device: &mut Device, (x, y): (i32, i32)) -> Result<()> {
    //     // trace!("Drawing at ({}, {})", x, y);
    //     goto_xy(device, (x, y))?;
    //     pen_down(device)?;
    //
    //     // Wiggle a little bit
    //     for n in 0..2 {
    //         goto_xy(device, (x + n, y + n))?;
    //     }
    //
    //     pen_up(device)?;
    //
    //     // sleep for 5ms
    //     thread::sleep(time::Duration::from_millis(1));
    //
    //     Ok(())
    // }

    pub fn pen_down(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EvdevEventType::KEY.0, 320, 1),           // BTN_TOOL_PEN
                InputEvent::new(EvdevEventType::KEY.0, 330, 1),           // BTN_TOUCH
                InputEvent::new(EvdevEventType::ABSOLUTE.0, 24, 2630),    // ABS_PRESSURE (max pressure)
                InputEvent::new(EvdevEventType::ABSOLUTE.0, 25, 0),       // ABS_DISTANCE
                InputEvent::new(EvdevEventType::SYNCHRONIZATION.0, 0, 0), // SYN_REPORT
            ])?;
        }
        Ok(())
    }

    pub fn pen_up(&mut self) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EvdevEventType::ABSOLUTE.0, 24, 0),       // ABS_PRESSURE
                InputEvent::new(EvdevEventType::ABSOLUTE.0, 25, 100),     // ABS_DISTANCE
                InputEvent::new(EvdevEventType::KEY.0, 330, 0),           // BTN_TOUCH
                InputEvent::new(EvdevEventType::KEY.0, 320, 0),           // BTN_TOOL_PEN
                InputEvent::new(EvdevEventType::SYNCHRONIZATION.0, 0, 0), // SYN_REPORT
            ])?;
        }
        Ok(())
    }

    pub fn goto_xy_virtual(&mut self, point: (i32, i32)) -> Result<()> {
        self.goto_xy(self.virtual_to_input(point))
    }

    fn goto_xy_virtual_f(&mut self, point: (f32, f32)) -> Result<()> {
        self.goto_xy(self.virtual_to_input_f(point))
    }

    pub fn goto_xy(&mut self, (x, y): (i32, i32)) -> Result<()> {
        if let Some(device) = &mut self.device {
            device.send_events(&[
                InputEvent::new(EvdevEventType::ABSOLUTE.0, 0, x),        // ABS_X
                InputEvent::new(EvdevEventType::ABSOLUTE.0, 1, y),        // ABS_Y
                InputEvent::new(EvdevEventType::SYNCHRONIZATION.0, 0, 0), // SYN_REPORT
            ])?;
        }
        Ok(())
    }

    pub fn max_x_value(&self) -> i32 {
        match self.device_model {
            DeviceModel::Remarkable2 => 15725,
            DeviceModel::RemarkablePaperPro => 11180,
            DeviceModel::Unknown => 15725, // Default to RM2
        }
    }

    pub fn max_y_value(&self) -> i32 {
        match self.device_model {
            DeviceModel::Remarkable2 => 20966,
            DeviceModel::RemarkablePaperPro => 15340,
            DeviceModel::Unknown => 20966, // Default to RM2
        }
    }

    fn virtual_to_input(&self, (x, y): (i32, i32)) -> (i32, i32) {
        self.virtual_to_input_f((x as f32, y as f32))
    }

    fn virtual_to_input_f(&self, (x, y): (f32, f32)) -> (i32, i32) {
        // Swap and normalize the coordinates
        let x_normalized = x / VIRTUAL_WIDTH as f32;
        let y_normalized = y / VIRTUAL_HEIGHT as f32;

        match self.device_model {
            DeviceModel::RemarkablePaperPro => {
                let x_input = (x_normalized * self.max_x_value() as f32) as i32;
                let y_input = (y_normalized * self.max_y_value() as f32) as i32;
                (x_input, y_input)
            }
            _ => {
                let x_input = ((1.0 - y_normalized) * self.max_y_value() as f32) as i32;
                let y_input = (x_normalized * self.max_x_value() as f32) as i32;
                (x_input, y_input)
            }
        }
    }
}
