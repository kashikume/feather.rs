use std::cell::Cell;
use cgmath::Deg;

use super::math::{Mat4, Vec3, Point3};
use super::camera::Camera;

pub struct PerspectiveCamera {
    projection: Cell<Option<Mat4>>,
    view: Cell<Option<Mat4>>,
    fov: f32,
    near: f32,
    far: f32,
    screen_width: u32,
    screen_height: u32,
    eye_position: Point3,
    target_position: Point3,
    up_vector: Vec3,
}

impl Camera for PerspectiveCamera {
    fn get_projection(&self) -> Mat4 {
        match self.projection.get() {
            Some(projection) => projection,
            None => {
                #[rustfmt::skip]
                let correction = Mat4::new(
                    1.0,  0.0,       0.0, 0.0,
                    0.0, -1.0,       0.0, 0.0,
                    0.0,  0.0, 1.0 / 2.0, 0.0,
                    0.0,  0.0, 1.0 / 2.0, 1.0,
                );
        
                let projection = correction
                    * cgmath::perspective(
                        Deg(self.fov),
                        self.screen_width as f32
                            / self.screen_height as f32,
                        self.near,
                        self.far,
                    );
                self.projection.set(Some(projection));
                projection
            }
        }
    }

    fn get_view(&self) -> Mat4 {
        match self.view.get() {
            Some(view) => view,
            None => {
                let view = Mat4::look_at_rh(self.eye_position, self.target_position , self.up_vector);
                self.view.set(Some(view));
                view
            }
        }
    }
}

impl PerspectiveCamera {

    pub fn new() -> Self {
        Self {
            projection: std::cell::Cell::new(None),
            view: std::cell::Cell::new(None),
            fov: 45.0,
            near: 0.1,
            far: 10.0,
            screen_width: 1920,
            screen_height: 1080,
            eye_position: Point3::new(2.0, 2.0, 2.0),
            target_position: Point3::new(0.0, 0.0, 0.0),
            up_vector: Vec3::new(0.0, 1.0, 0.0),
        }
    }

    pub fn set_screen_dimention(&mut self, width: u32, height: u32) -> &mut Self {
        if width != self.screen_width || height != self.screen_height {
            self.screen_width = width;
            self.screen_height = height;
            self.projection.set(None);
        }
        self
    }

    pub fn set_fov(&mut self, fov: f32) -> &mut Self {
        if fov != self.fov {
            self.fov = fov;
            self.projection.set(None);
        }
        self
    }

    pub fn set_near_far(&mut self, near: f32, far: f32) -> &mut Self {
        if near != self.near || far != self.far {
            self.near = near;
            self.far = far;
            self.projection.set(None);
        }
        self
    }

    pub fn set_near(&mut self, near: f32) -> &mut Self {
        if near != self.near {
            self.near = near;
            self.projection.set(None);
        }
        self
    }
    
    pub fn set_far(&mut self, far: f32) -> &mut Self {
        if far != self.far {
            self.far = far;
            self.projection.set(None);
        }
        self
    }

    pub fn set_view(&mut self, eye_position: Point3, target_position: Point3, up_vector: Vec3) -> &mut Self {
        if eye_position != self.eye_position || target_position != self.target_position || up_vector != self.up_vector {
            self.eye_position = eye_position;
            self.target_position = target_position;
            self.up_vector = up_vector;
            self.view.set(None);
        }
        self
    }

    pub fn set_eye_position(&mut self, eye_position: Point3) -> &mut Self {
        if eye_position != self.eye_position {
            self.eye_position = eye_position;
            self.view.set(None);
        }
        self
    }

    pub fn set_target_position(&mut self, target_position: Point3) -> &mut Self {
        if target_position != self.target_position {
            self.target_position = target_position;
            self.view.set(None);
        }
        self
    }

    pub fn set_up_vector(&mut self, up_vector: Vec3) -> &mut Self {
        if up_vector != self.up_vector {
            self.up_vector = up_vector;
            self.view.set(None);
        }
        self
    }
}

impl Default for PerspectiveCamera {
    fn default() -> Self {
        Self::new()
    }
}
