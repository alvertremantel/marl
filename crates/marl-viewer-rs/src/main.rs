use std::error::Error;

use winit::event_loop::EventLoop;

mod app;
mod args;
mod io;
mod renderer;

use app::ViewerApp;
use args::ViewerArgs;
use io::load_field;

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
    let payload = load_field(&args)?;
    eprintln!(
        "[viewer] loaded tick {} species {} from {} ({} bytes)",
        payload.tick,
        payload.species,
        args.output_dir.display(),
        payload.bytes.len()
    );

    let event_loop = EventLoop::new()?;
    let app = ViewerApp::new(payload);
    app.run(event_loop)
}
