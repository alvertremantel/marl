use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::io::FieldPayload;
use crate::renderer::{RenderResult, Renderer};

pub(crate) struct ViewerApp {
    payload: Option<FieldPayload>,
    renderer: Option<Renderer>,
}

impl ViewerApp {
    pub(crate) fn new(payload: FieldPayload) -> Self {
        Self {
            payload: Some(payload),
            renderer: None,
        }
    }

    pub(crate) fn run(self, event_loop: EventLoop<()>) -> Result<(), Box<dyn std::error::Error>> {
        let mut app = self;
        event_loop.run_app(&mut app)?;
        Ok(())
    }
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let Some(payload) = self.payload.take() else {
            event_loop.exit();
            return;
        };

        let attrs = Window::default_attributes()
            .with_title("MARL Viewer")
            .with_inner_size(PhysicalSize::new(1280, 720));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                eprintln!("failed to create viewer window: {e}");
                event_loop.exit();
                return;
            }
        };

        match pollster::block_on(Renderer::new(window, payload)) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(e) => {
                eprintln!("failed to initialize viewer renderer: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if window_id != renderer.window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => renderer.resize(renderer.window.inner_size()),
            WindowEvent::RedrawRequested => match renderer.render() {
                RenderResult::Drawn | RenderResult::Skip => {}
                RenderResult::Reconfigure => renderer.resize(renderer.window.inner_size()),
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.window.request_redraw();
        }
    }
}
