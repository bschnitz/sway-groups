//! Minimal Wayland window for integration testing.
//!
//! Opens a 200x100 colored window with a configurable `app_id`, then blocks
//! until SIGTERM or SIGINT. Sway registers the window immediately and removes
//! it when the process exits.
//!
//! Usage: sway-dummy-window <app_id>

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::delegate_compositor;
use smithay_client_toolkit::delegate_output;
use smithay_client_toolkit::delegate_registry;
use smithay_client_toolkit::delegate_shm;
use smithay_client_toolkit::delegate_xdg_shell;
use smithay_client_toolkit::delegate_xdg_window;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::{wl_output, wl_shm, wl_surface};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};

const WIDTH: u32 = 200;
const HEIGHT: u32 = 100;

struct DummyWindow {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    xdg_shell: XdgShell,
    window: Option<Window>,
    running: Arc<AtomicBool>,
    first_configure: bool,
    shm: Shm,
    pool: SlotPool,
    buffer: Option<Buffer>,
}

impl ShmHandler for DummyWindow {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for DummyWindow {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

delegate_output!(DummyWindow);

impl CompositorHandler for DummyWindow {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

delegate_compositor!(DummyWindow);

impl WindowHandler for DummyWindow {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn configure(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        _: WindowConfigure,
        _: u32,
    ) {
        self.buffer = None;
        if self.first_configure {
            self.first_configure = false;
            self.draw(conn, qh);
        }
    }
}

delegate_xdg_shell!(DummyWindow);
delegate_xdg_window!(DummyWindow);
delegate_shm!(DummyWindow);

impl ProvidesRegistryState for DummyWindow {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState];
}

delegate_registry!(DummyWindow);

impl DummyWindow {
    fn draw(&mut self, _conn: &Connection, qh: &QueueHandle<Self>) {
        let width = WIDTH;
        let height = HEIGHT;
        let stride = width as i32 * 4;

        let buffer = self.buffer.get_or_insert_with(|| {
            self.pool
                .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
                .expect("create buffer")
                .0
        });

        let canvas = match self.pool.canvas(buffer) {
            Some(canvas) => canvas,
            None => {
                let (second_buffer, canvas) = self
                    .pool
                    .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
                    .expect("create buffer");
                *buffer = second_buffer;
                canvas
            }
        };

        canvas.chunks_exact_mut(4).for_each(|chunk| {
            let color: u32 = 0xFF80_4040;
            let array: &mut [u8; 4] = chunk.try_into().unwrap();
            *array = color.to_le_bytes();
        });

        if let Some(window) = &self.window {
            let surface = window.wl_surface();
            surface.damage_buffer(0, 0, width as i32, height as i32);
            surface.frame(qh, surface.clone());
            buffer.attach_to(surface).expect("buffer attach");
            window.commit();
        }
    }
}

// --- Signal handling ---

static RUNNING_PTR: std::sync::atomic::AtomicPtr<()> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

extern "C" fn handle_signal(_: libc::c_int) {
    let ptr = RUNNING_PTR.load(Ordering::SeqCst);
    if !ptr.is_null() {
        let flag = unsafe { &*(ptr as *const AtomicBool) };
        flag.store(false, Ordering::SeqCst);
    }
}

// --- Main ---

fn main() {
    let app_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sway-dummy-window".to_string());

    let running = Arc::new(AtomicBool::new(true));
    let running_ptr = Arc::into_raw(running.clone()) as *mut ();
    RUNNING_PTR.store(running_ptr, Ordering::SeqCst);

    unsafe {
        libc::signal(libc::SIGTERM, handle_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, handle_signal as *const () as libc::sighandler_t);
    }

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland display");
    let (globals, event_queue) = registry_queue_init(&conn).expect("Failed to init registry");
    let qh = event_queue.handle();

    let compositor_state = CompositorState::bind(&globals, &qh).expect("wl_compositor not available");
    let output_state = OutputState::new(&globals, &qh);
    let xdg_shell = XdgShell::bind(&globals, &qh).expect("xdg_wm_base not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm not available");

    let pool = SlotPool::new(WIDTH as usize * HEIGHT as usize * 4, &shm)
        .expect("Failed to create slot pool");

    let surface = compositor_state.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, &qh);
    window.set_app_id(app_id);
    window.set_title("sway-dummy-window");
    window.set_min_size(Some((WIDTH, HEIGHT)));
    window.commit();

    let mut state = DummyWindow {
        registry_state: RegistryState::new(&globals),
        output_state,
        compositor_state,
        xdg_shell,
        window: Some(window),
        running: running.clone(),
        first_configure: true,
        shm,
        pool,
        buffer: None,
    };

    let mut event_loop: EventLoop<DummyWindow> = EventLoop::try_new().expect("Failed to create event loop");
    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .expect("Failed to insert Wayland source");

    while running.load(Ordering::SeqCst) {
        event_loop.dispatch(Some(std::time::Duration::from_millis(50)), &mut state).expect("Event loop error");
    }

    unsafe { drop(Arc::from_raw(running_ptr as *const AtomicBool)) };
}
