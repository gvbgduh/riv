//! # Program
//!
//! Program contains the program struct, which contains all information needed to run the
//! event loop and render the images to screen

use crate::cli;
use crate::ui::{self, Action};
use fs_extra::file::move_file;
use sdl2::image::LoadTexture;
use sdl2::rect::Rect;
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::video::WindowContext;
use sdl2::Sdl;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

/// Program contains all information needed to run the event loop and render the images to screen
pub struct Program {
    sdl_context: Sdl,
    canvas: WindowCanvas,
    texture_creator: TextureCreator<WindowContext>,
    images: Vec<PathBuf>,
    dest_folder: PathBuf,
    index: usize,
    ui_state: ui::State,
}

impl Program {
    /// init scaffolds the program, by making a call to the cli module to parse the command line arguments,
    /// sets up the sdl context, creates the window, the canvas and the texture creator.
    pub fn init() -> Result<Program, String> {
        let args = cli::cli()?;
        let images = args.files;
        let dest_folder = args.dest_folder;
        let sdl_context = sdl2::init()?;
        let video = sdl_context.video()?;
        let window = video
            .window(
                "rust-sdl2 demo: Video",
                video.display_bounds(0).unwrap().width(),
                video.display_bounds(0).unwrap().height(),
            )
            .position_centered()
            .resizable()
            .build()
            .map_err(|e| e.to_string())?;

        let canvas = window
            .into_canvas()
            .software()
            .build()
            .map_err(|e| e.to_string())?;
        let texture_creator = canvas.texture_creator();
        let ui_state = ui::State {
            left_shift: false,
            right_shift: false,
        };
        Ok(Program {
            sdl_context,
            canvas,
            texture_creator,
            images,
            dest_folder,
            index: 0,
            ui_state,
        })
    }

    /// render loads the image at the path in the images path vector located at the index and renders to screen
    pub fn render(&mut self) -> Result<(), String> {
        if self.images.is_empty() {
            return Ok(());
        }
        let texture = match self.texture_creator.load_texture(&self.images[self.index]) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("failed to render image {}", e);
                return Ok(());
            }
        };
        let query = texture.query();
        let target = self.canvas.viewport();
        let dest = make_dst(query.width, query.height, target.width(), target.height());
        self.canvas.clear();
        if let Err(e) = self.canvas.copy(&texture, None, dest) {
            eprintln!("Failed to copy image to screen {}", e);
            return Ok(());
        }
        self.canvas.present();
        Ok(())
    }

    fn increment(&mut self, step: usize) -> Result<(), String> {
        if self.images.is_empty() || self.images.len() == 1 {
            return Ok(());
        }
        if self.index < self.images.len() - step {
            self.index += step;
        }
        self.render()
    }

    fn decrement(&mut self, step: usize) -> Result<(), String> {
        if self.index >= step {
            self.index -= step;
        }
        self.render()
    }

    fn first(&mut self) -> Result<(), String> {
        self.index = 0;
        self.render()
    }

    fn last(&mut self) -> Result<(), String> {
        if self.images.is_empty() {
            self.index = 0;
        } else {
            self.index = self.images.len() - 1;
        }
        self.render()
    }

    fn move_image(&mut self) -> Result<(), String> {
        match std::fs::create_dir_all(&self.dest_folder) {
            Ok(_) => (),
            Err(e) => match e.kind() {
                ErrorKind::AlreadyExists => (),
                _ => return Err(e.to_string()),
            },
        };
        let filepath = self.images.remove(self.index);
        if self.index >= self.images.len() && !self.images.is_empty() {
            self.index -= 1;
        }
        let filename = match filepath.file_name() {
            Some(f) => f,
            None => return Err("failed to read filename for current image".to_string()),
        };
        let newname = PathBuf::from(&self.dest_folder).join(filename);
        let opt = &fs_extra::file::CopyOptions::new();
        move_file(filepath, newname, opt).map_err(|e| e.to_string())?;
        self.render()
    }

    /// run is the event loop that listens for input and delegates accordingly.
    pub fn run(&mut self) -> Result<(), String> {
        self.render()?;

        'mainloop: loop {
            for event in self.sdl_context.event_pump()?.poll_iter() {
                match ui::event_action(&mut self.ui_state, &event) {
                    Action::Quit => break 'mainloop,
                    Action::ReRender => self.render()?,
                    Action::Next => self.increment(1)?,
                    Action::Prev => self.decrement(1)?,
                    Action::Move => match self.move_image() {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to move file: {}", e),
                    },
                    Action::First => self.first()?,
                    Action::Last => self.last()?,
                    Action::Noop => {}
                }
            }
            std::thread::sleep(Duration::from_millis(0));
        }

        Ok(())
    }
}

/// make dst determines the parameters of a rectangle required to place an image correctly in
/// the window
fn make_dst(src_x: u32, src_y: u32, dst_x: u32, dst_y: u32) -> Rect {
    // case 1: both source dimensions smaller
    if src_x < dst_x && src_y < dst_y {
        return full_rect(src_x, src_y, dst_x, dst_y);
    }
    // case 2: source aspect ratio is larger
    if src_x as f32 / src_y as f32 > dst_x as f32 / dst_y as f32 {
        return fit_x_rect(src_x, src_y, dst_x, dst_y);
    }
    // case 3: source aspect ratio is smaller
    fit_y_rect(src_x, src_y, dst_x, dst_y)
}

fn full_rect(src_x: u32, src_y: u32, dst_x: u32, dst_y: u32) -> Rect {
    let y = ((dst_y - src_y) as f32 / 2.0) as i32;
    let x = ((dst_x - src_x) as f32 / 2.0) as i32;
    Rect::new(x, y, src_x, src_y)
}

fn fit_x_rect(src_x: u32, src_y: u32, dst_x: u32, dst_y: u32) -> Rect {
    let height = ((src_y as f32 / src_x as f32) * dst_x as f32) as u32;
    let y = ((dst_y - height) as f32 / 2.0) as i32;
    Rect::new(0, y, dst_x, height)
}

fn fit_y_rect(src_x: u32, src_y: u32, dst_x: u32, dst_y: u32) -> Rect {
    let width = ((src_x as f32 / src_y as f32) * dst_y as f32) as u32;
    let x = ((dst_x - width) as f32 / 2.0) as i32;
    Rect::new(x, 0, width, dst_y)
}
