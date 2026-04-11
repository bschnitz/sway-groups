//! Minimal Wayland window for integration testing.
//!
//! Opens a single window with a configurable `app_id`, then blocks until
//! SIGTERM or SIGINT. Sway registers the window immediately and removes it
//! when the process exits.
//!
//! Usage: sway-dummy-window <app_id>

use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::delegate_compositor;
use smithay_client_toolkit::delegate_output;
use smithay_client_toolkit::delegate_registry;
use smithay_client_toolkit::delegate_xdg_shell;
use smithay_client_toolkit::delegate_xdg_window;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::{wl_buffer, wl_output, wl_shm, wl_shm_pool, wl_surface};
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, Proxy, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shell::xdg::window::{
    Window, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::WaylandSurface;

struct DummyWindow {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    xdg_shell: XdgShell,
    window: Option<Window>,
    running: Arc<AtomicBool>,
    configured: bool,
    wl_shm: wl_shm::WlShm,
}

impl Dispatch<wl_shm::WlShm, ()> for DummyWindow {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for DummyWindow {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: <wl_shm_pool::WlShmPool as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for DummyWindow {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: <wl_buffer::WlBuffer as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

// --- OutputHandler ---

impl OutputHandler for DummyWindow {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

delegate_output!(DummyWindow);

// --- CompositorHandler ---

impl CompositorHandler for DummyWindow {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

delegate_compositor!(DummyWindow);

// --- WindowHandler ---

impl WindowHandler for DummyWindow {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &Window,
        _: WindowConfigure,
        _: u32,
    ) {
        if !self.configured {
            self.configured = true;
            if let Some(window) = &self.window {
                let surface = window.wl_surface();
                let fd = create_memfd();
                let pool = self.wl_shm.create_pool(fd.as_fd(), 4, qh, ());
                let buffer = pool.create_buffer(0, 1, 1, 4, wl_shm::Format::Argb8888, qh, ());
                surface.attach(Some(&buffer), 0, 0);
                surface.commit();
            }
        }
    }
}

delegate_xdg_shell!(DummyWindow);
delegate_xdg_window!(DummyWindow);

// --- Registry ---

impl ProvidesRegistryState for DummyWindow {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState];
}

delegate_registry!(DummyWindow);

fn create_memfd() -> OwnedFd {
    use std::ffi::CStr;
    let name = CStr::from_bytes_with_nul(b"sway-dummy-window\0").unwrap();
    let fd = unsafe {
        libc::syscall(libc::SYS_memfd_create, name.as_ptr(), libc::MFD_CLOEXEC) as i32
    };
    if fd < 0 {
        panic!("memfd_create failed: {}", std::io::Error::last_os_error());
    }
    unsafe {
        libc::ftruncate(fd, 4);
        FromRawFd::from_raw_fd(fd)
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
    let wl_shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).expect("wl_shm not available");

    let surface = compositor_state.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, &qh);
    window.set_app_id(app_id);
    window.set_title("sway-dummy-window");
    window.set_min_size(Some((1, 1)));
    window.commit();

    let mut state = DummyWindow {
        registry_state: RegistryState::new(&globals),
        output_state,
        compositor_state,
        xdg_shell,
        window: Some(window),
        running: running.clone(),
        configured: false,
        wl_shm,
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
