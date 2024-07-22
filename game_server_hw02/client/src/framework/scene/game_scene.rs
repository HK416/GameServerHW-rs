use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};
use cgmath::{Vector2, Vector3, Point3};
use futures::executor::block_on;
use std::{
    net::TcpStream,
    io::{Read, Write},
};

use super::super::{
    camera::{Camera, CameraComponent, DefaultCamera},
    object::Object,
    model::Model,
    color::Color,
    SCREEN_WIDTH, SCREEN_HEIGHT,
};
use super::Scene;


pub struct GameScene {
    camera: DefaultCamera,
    camera_offset: Vector3<f32>,

    background_color: Color,

    models: Vec<Model>,
    objects: Vec<Object>,

    player: *mut Object,

    // ip: String,
    // port: u16,
    addr: String,
    stream: TcpStream,
}

impl GameScene {
    pub async fn new() -> Self {
        let camera = DefaultCamera::from(CameraComponent {
            eye: Point3::new(0.0, 1.0, 2.0),
            target: Point3::new(0.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            aspect: SCREEN_WIDTH as f32 / SCREEN_HEIGHT as f32,
            fovy: 60.0,
            znear: 0.1,
            zfar: 100.0,
        });

        let ip = "127.0.0.1".to_string();
        let port = 8080;
        let addr = format!("{}:{}", ip, port);
        let stream = TcpStream::connect(addr.clone()).unwrap();
        stream.set_nonblocking(true).unwrap();

        Self {
            camera,
            camera_offset: Vector3::new(0.0, 2.0, 4.0),

            background_color: Color::BLACK,

            models: Vec::new(),
            objects: Vec::new(),

            player: std::ptr::null_mut(),

            // ip,
            // port,
            addr,
            stream,
        }
    }

    fn load_models(&mut self, device: &wgpu::Device) {
        block_on(async {
            self.models = vec![
                Model::load("cube.obj", device, 0.5, Color::LIGHT_GRAY).await.unwrap(),
                Model::load("cube.obj", device, 0.5, Color::DARK_GRAY).await.unwrap(),
                Model::load("pawn.obj", device, 0.8, Color::WHITE).await.unwrap(),
                Model::load("pawn.obj", device, 0.8, Color::BLACK).await.unwrap(),
            ];
        });
    }

    fn build_objects(&mut self) {
        self.objects = (0..64)
            .map(|_| Object::new())
            .collect::<Vec<_>>();

        for (i, object) in self.objects.iter_mut().enumerate() {
            let x = i % 8;
            let z = i / 8;
            object.transform.position = Vector3::new(
                x as f32,
                -0.5,
                z as f32
            );
            object.set_model(&mut self.models[(x+z) & 1]);
        }

        self.objects.push(Object::new());
        
        let p = self.objects.last_mut().unwrap();
        p.set_model(&mut self.models[2]);
        p.transform.position = Vector3::new(
            0.0,
            0.0,
            0.0,
        );

        self.player = p;
    }

    fn player(&self) -> &mut Object {
        unsafe { &mut *self.player }
    }

    fn update_camera(&mut self) {
        let p = self.player().transform.position;

        let point = Point3::new(p.x, p.y, p.z);

        self.camera.component.target = point;
        self.camera.component.eye = point + self.camera_offset;
    }

    fn pull_messages(&mut self) -> Option<String> {
        let mut buf = [0; 1024];

        match self.stream.read(&mut buf) {
            Ok(0) => {
                println!("Connection closed");
                None
            },
            Ok(n) => {
                let msg = String::from_utf8_lossy(&buf[..n]);
                // println!("Received: {}", msg);
                Some(msg.to_string())
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // println!("Would block");
                None
            },
            Err(e) => {
                eprintln!("Failed to read from socket; err = {:?}", e);
                match TcpStream::connect(self.addr.clone()) {
                    Ok(stream) => {
                        self.stream = stream;
                        self.stream.set_nonblocking(true).unwrap();
                        self.pull_messages()
                    },
                    Err(e) => {
                        eprintln!("Failed to reconnect; err = {:?}", e);
                        None
                    }
                }
            }
        }
    }

    fn process_message(&mut self, msg: &str) {
        let msg = msg.trim().split_whitespace()
            .map(|s| s.trim())
            .collect::<Vec<&str>>();

        println!("Received: {:?}", msg);

        if msg.len() == 0 {
            return;
        }

        match msg[0] {
            "move" => {
                if msg.len() != 3 {
                    return;
                }

                let x = msg[1].parse::<i32>().unwrap_or(0);
                let y = msg[2].parse::<i32>().unwrap_or(0);

                let p = self.player();
                p.transform.position.x += x as f32;
                p.transform.position.z += y as f32;
            },
            "set" => {
                if msg.len() != 3 {
                    return;
                }

                let x = msg[1].parse::<i32>().unwrap_or(0);
                let y = msg[2].parse::<i32>().unwrap_or(0);

                let p = self.player();
                p.transform.position.x = x as f32;
                p.transform.position.z = y as f32;

                println!("set ok");
            },
            _ => {}
        }
    }

    fn process_messages(&mut self, msg: &str) {
        let messages = msg.trim().split("\n").collect::<Vec<&str>>();

        println!("messages: {:?}", messages);

        for msg in messages {
            self.process_message(msg);
        }
    }

    fn process_keyboard_input(&mut self, state: &ElementState, keycode: &KeyCode) -> bool {
        match state {
            ElementState::Pressed => {
                let mut direction = Vector2::new(0, 0);

                match keycode {
                    KeyCode::KeyW => direction.y = -1,
                    KeyCode::KeyA => direction.x = -1,
                    KeyCode::KeyS => direction.y = 1,
                    KeyCode::KeyD => direction.x = 1,
                    _ => return false,
                }
                
                println!("Move ({} {})", direction.x, direction.y);

                // println!("{}", self.stream.peer_addr().unwrap());
                let msg = format!("move {} {}\n", direction.x, direction.y);
                self.stream.write_all(msg.as_bytes())
                    .expect("Failed to write to stream");

                println!("Sent ok");

                true
            }
            ElementState::Released => false
        }
    }
}

impl Scene for GameScene {
    fn init(&mut self, device: &wgpu::Device) {
        self.load_models(device);
        self.build_objects();
    }

    fn handle_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    state,
                    physical_key: PhysicalKey::Code(keycode),
                    repeat: false,
                    ..
                },
                ..
            } => self.process_keyboard_input(state, keycode),
            _ => false,
        }
    }

    fn update(&mut self) {
        while let Some(msg) = self.pull_messages() {
            self.process_messages(&msg);
        }

        self.update_camera();
    }


    fn view_proj(&self) -> cgmath::Matrix4<f32> {
        self.camera.build_view_projection_matrix()
    }

    fn models(&self) -> &Vec<Model> {
        &self.models
    }

    fn objects(&self) -> &Vec<Object> {
        &self.objects
    }

    fn background_color(&self) -> Color {
        self.background_color
    }
}
