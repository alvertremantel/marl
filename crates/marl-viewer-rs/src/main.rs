use std::error::Error;

use winit::event_loop::EventLoop;

mod app;
mod args;
mod camera;
mod io;
mod renderer;

use app::ViewerApp;
use args::ViewerArgs;
use io::load_snapshot;

#[cfg(not(target_endian = "little"))]
compile_error!("the MARL viewer expects little-endian f32 field dumps");

fn main() -> Result<(), Box<dyn Error>> {
    let args = match ViewerArgs::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(if e.starts_with("Usage:") { 0 } else { 2 });
        }
    };
    let payload = load_snapshot(&args)?;
    eprintln!(
        "[viewer] loaded tick {} species {} view {:?} cells {:?} ({} field bytes, {} cells)",
        payload.tick,
        payload.species,
        args.view_mode,
        args.cell_mode,
        payload.field_bytes.len(),
        payload.cells.len()
    );

    let event_loop = EventLoop::new()?;
    let app = ViewerApp::new(payload, args);
    app.run(event_loop)
}
