use std::ffi::c_void;
use std::ptr;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D9::{
    D3D_SDK_VERSION, D3DADAPTER_DEFAULT, D3DCLEAR_STENCIL, D3DCLEAR_TARGET, D3DCLEAR_ZBUFFER,
    D3DCREATE_HARDWARE_VERTEXPROCESSING, D3DCREATE_SOFTWARE_VERTEXPROCESSING, D3DDEVTYPE_HAL,
    D3DFMT_D24S8, D3DFMT_UNKNOWN, D3DMULTISAMPLE_NONE, D3DPRESENT_INTERVAL_ONE,
    D3DPRESENT_PARAMETERS, D3DSWAPEFFECT_DISCARD, Direct3DCreate9, IDirect3D9, IDirect3DDevice9,
};
use windows::core::{BOOL, Error, HRESULT, Result};
use winit::dpi::PhysicalSize;
use winit::window::Window;

const E_FAIL: HRESULT = HRESULT(0x8000_4005_u32 as i32);
const D3DERR_DEVICELOST: HRESULT = HRESULT(0x8876_0868_u32 as i32);
const D3DERR_DEVICENOTRESET: HRESULT = HRESULT(0x8876_0869_u32 as i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D3d9FrameStatus {
    Presented,
    DeviceLost,
    Minimized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D3d9DeviceStatus {
    Ready,
    NeedsReset,
    DeviceLost,
    Minimized,
}

/// Owns the native Direct3D 9 interface, device and reset parameters.
///
/// The wrapper intentionally uses only `d3d9.dll`; it does not load D3DX or
/// any NVIDIA Cg runtime. COM interface widths are supplied by `windows-rs`,
/// so the same source builds for every Windows host architecture supported by
/// the Rust toolchain.
pub struct D3d9Device {
    _direct3d: IDirect3D9,
    device: IDirect3DDevice9,
    present: D3DPRESENT_PARAMETERS,
    size: PhysicalSize<u32>,
    reset_pending: bool,
    device_lost: bool,
}

impl D3d9Device {
    pub fn new(window: &Window) -> Result<Self> {
        let hwnd = hwnd_from_window(window)?;
        let size = window.inner_size();
        let mut present = make_present_parameters(hwnd, size);
        let direct3d = unsafe { Direct3DCreate9(D3D_SDK_VERSION) }.ok_or_else(|| {
            Error::new(
                E_FAIL,
                "Direct3DCreate9 returned a null IDirect3D9 interface",
            )
        })?;

        let device = match create_device(
            &direct3d,
            hwnd,
            &mut present,
            D3DCREATE_HARDWARE_VERTEXPROCESSING as u32,
        ) {
            Ok(device) => device,
            Err(hardware_error) => create_device(
                &direct3d,
                hwnd,
                &mut present,
                D3DCREATE_SOFTWARE_VERTEXPROCESSING as u32,
            )
            .map_err(|software_error| {
                Error::new(
                    software_error.code(),
                    format!(
                        "IDirect3D9::CreateDevice failed with hardware vertex processing ({hardware_error}) and software vertex processing ({software_error})"
                    ),
                )
            })?,
        };

        Ok(Self {
            _direct3d: direct3d,
            device,
            present,
            size,
            reset_pending: false,
            device_lost: false,
        })
    }

    pub fn device(&self) -> &IDirect3DDevice9 {
        &self.device
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        if size.width != 0 && size.height != 0 {
            self.present.BackBufferWidth = size.width;
            self.present.BackBufferHeight = size.height;
            self.reset_pending = true;
        }
    }

    pub fn render_clear_frame(&mut self, clear_argb: u32) -> Result<D3d9FrameStatus> {
        match self.status()? {
            D3d9DeviceStatus::Ready => {}
            D3d9DeviceStatus::NeedsReset => self.reset()?,
            D3d9DeviceStatus::DeviceLost => return Ok(D3d9FrameStatus::DeviceLost),
            D3d9DeviceStatus::Minimized => return Ok(D3d9FrameStatus::Minimized),
        }

        self.clear_and_begin_scene(clear_argb)?;
        self.end_scene_and_present()
    }

    pub fn status(&mut self) -> Result<D3d9DeviceStatus> {
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(D3d9DeviceStatus::Minimized);
        }

        if self.device_lost {
            match unsafe { self.device.TestCooperativeLevel() } {
                Ok(()) => self.device_lost = false,
                Err(error) if error.code() == D3DERR_DEVICELOST => {
                    return Ok(D3d9DeviceStatus::DeviceLost);
                }
                Err(error) if error.code() == D3DERR_DEVICENOTRESET => {
                    self.reset_pending = true;
                }
                Err(error) => return Err(error),
            }
        }
        if self.reset_pending {
            Ok(D3d9DeviceStatus::NeedsReset)
        } else {
            Ok(D3d9DeviceStatus::Ready)
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        unsafe { self.device.Reset(&mut self.present)? };
        self.reset_pending = false;
        self.device_lost = false;
        Ok(())
    }

    pub fn clear_and_begin_scene(&self, clear_argb: u32) -> Result<()> {
        unsafe {
            self.device.Clear(
                0,
                ptr::null(),
                (D3DCLEAR_TARGET | D3DCLEAR_ZBUFFER | D3DCLEAR_STENCIL) as u32,
                clear_argb,
                1.0,
                0,
            )?;
            self.device.BeginScene()
        }
    }

    pub fn end_scene_and_present(&mut self) -> Result<D3d9FrameStatus> {
        unsafe { self.device.EndScene()? };

        match unsafe {
            self.device
                .Present(ptr::null(), ptr::null(), HWND::default(), ptr::null())
        } {
            Ok(()) => Ok(D3d9FrameStatus::Presented),
            Err(error) if error.code() == D3DERR_DEVICELOST => {
                self.device_lost = true;
                Ok(D3d9FrameStatus::DeviceLost)
            }
            Err(error) => Err(error),
        }
    }
}

fn hwnd_from_window(window: &Window) -> Result<HWND> {
    let handle = window
        .window_handle()
        .map_err(|error| Error::new(E_FAIL, format!("failed to obtain Win32 HWND: {error}")))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as usize as *mut c_void)),
        _ => Err(Error::new(
            E_FAIL,
            "the D3D9 editor requires a Win32 window handle",
        )),
    }
}

fn make_present_parameters(hwnd: HWND, size: PhysicalSize<u32>) -> D3DPRESENT_PARAMETERS {
    D3DPRESENT_PARAMETERS {
        BackBufferWidth: size.width.max(1),
        BackBufferHeight: size.height.max(1),
        BackBufferFormat: D3DFMT_UNKNOWN,
        BackBufferCount: 1,
        MultiSampleType: D3DMULTISAMPLE_NONE,
        MultiSampleQuality: 0,
        SwapEffect: D3DSWAPEFFECT_DISCARD,
        hDeviceWindow: hwnd,
        Windowed: BOOL(1),
        EnableAutoDepthStencil: BOOL(1),
        AutoDepthStencilFormat: D3DFMT_D24S8,
        Flags: 0,
        FullScreen_RefreshRateInHz: 0,
        PresentationInterval: D3DPRESENT_INTERVAL_ONE as u32,
    }
}

fn create_device(
    direct3d: &IDirect3D9,
    hwnd: HWND,
    present: &mut D3DPRESENT_PARAMETERS,
    behavior_flags: u32,
) -> Result<IDirect3DDevice9> {
    let mut device = None;
    unsafe {
        direct3d.CreateDevice(
            D3DADAPTER_DEFAULT,
            D3DDEVTYPE_HAL,
            hwnd,
            behavior_flags,
            present,
            &mut device,
        )?;
    }
    device.ok_or_else(|| Error::new(E_FAIL, "CreateDevice returned a null IDirect3DDevice9"))
}
