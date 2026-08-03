#![cfg_attr(not(target_os = "windows"), allow(dead_code, unused_imports))]

use srd_editor::shader::{
    CEYLON_SIMPLE_SHADER_KEY_LENGTH, CeylonSimpleShaderBits, decode_embedded_shader_source,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{CStr, c_char, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;

#[derive(Clone, Copy)]
struct SourceRecord {
    name: &'static str,
    virtual_address: u32,
    byte_count: usize,
    resource_hash: u32,
}

const SOURCE_RECORDS: [SourceRecord; 13] = [
    SourceRecord {
        name: "SimpleShaderPS.cg",
        virtual_address: 0x17c3_c60,
        byte_count: 0x2884,
        resource_hash: 0x5273_75cc,
    },
    SourceRecord {
        name: "SimpleShaderVS.cg",
        virtual_address: 0x17c6_4f0,
        byte_count: 0x1d98,
        resource_hash: 0x68d9_8052,
    },
    SourceRecord {
        name: "DefaultColorCompress.h",
        virtual_address: 0x17d2_228,
        byte_count: 0x2494,
        resource_hash: 0x3faf_b6e9,
    },
    SourceRecord {
        name: "DefaultShadow.h",
        virtual_address: 0x17ec_f88,
        byte_count: 0x10e0,
        resource_hash: 0x6635_5c90,
    },
    SourceRecord {
        name: "FixedPSUniform.h",
        virtual_address: 0x17f7_650,
        byte_count: 0x0230,
        resource_hash: 0xa5f0_c0b2,
    },
    SourceRecord {
        name: "FixedVSUniform.h",
        virtual_address: 0x17f7_888,
        byte_count: 0x0290,
        resource_hash: 0x7e81_3e3f,
    },
    SourceRecord {
        name: "ParticleSystem.h",
        virtual_address: 0x17f7_b20,
        byte_count: 0x41ec,
        resource_hash: 0x65b4_7d15,
    },
    SourceRecord {
        name: "PhotoShopLayerBlendPS.h",
        virtual_address: 0x17fb_d10,
        byte_count: 0x2c3c,
        resource_hash: 0x4a65_fbf5,
    },
    SourceRecord {
        name: "SimpleCommonFunc.h",
        virtual_address: 0x17fe_958,
        byte_count: 0x04f4,
        resource_hash: 0x2674_31f4,
    },
    SourceRecord {
        name: "SimplePSFunc.h",
        virtual_address: 0x17fe_e50,
        byte_count: 0x09a8,
        resource_hash: 0xc163_17df,
    },
    SourceRecord {
        name: "SimplePSUniform.h",
        virtual_address: 0x17ff_800,
        byte_count: 0x0998,
        resource_hash: 0xe477_76fd,
    },
    SourceRecord {
        name: "SimpleShaderDefine.h",
        virtual_address: 0x1800_1a0,
        byte_count: 0x0bb4,
        resource_hash: 0xf415_d5a2,
    },
    SourceRecord {
        name: "SimpleVSUniform.h",
        virtual_address: 0x1800_d60,
        byte_count: 0x0858,
        resource_hash: 0x876a_4c71,
    },
];

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("u16 outside file at 0x{offset:x}"))?;
    Ok(u16::from_le_bytes(value.try_into().unwrap()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("u32 outside file at 0x{offset:x}"))?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}

fn virtual_address_to_file_offset(bytes: &[u8], address: u32) -> Result<usize, String> {
    if bytes.get(0..2) != Some(b"MZ") {
        return Err("input is not an MZ executable".into());
    }
    let pe = read_u32(bytes, 0x3c)? as usize;
    if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("input has no PE signature".into());
    }
    let section_count = usize::from(read_u16(bytes, pe + 6)?);
    let optional_size = usize::from(read_u16(bytes, pe + 20)?);
    let optional = pe + 24;
    if read_u16(bytes, optional)? != 0x10b {
        return Err("expected the game's PE32 optional header".into());
    }
    let image_base = read_u32(bytes, optional + 28)?;
    let rva = address
        .checked_sub(image_base)
        .ok_or_else(|| format!("VA 0x{address:x} precedes image base 0x{image_base:x}"))?;
    let sections = optional + optional_size;
    for index in 0..section_count {
        let section = sections + index * 40;
        let virtual_size = read_u32(bytes, section + 8)?;
        let virtual_address = read_u32(bytes, section + 12)?;
        let raw_size = read_u32(bytes, section + 16)?;
        let raw_offset = read_u32(bytes, section + 20)?;
        let mapped_size = virtual_size.max(raw_size);
        if rva >= virtual_address && rva < virtual_address.saturating_add(mapped_size) {
            return Ok((raw_offset + (rva - virtual_address)) as usize);
        }
    }
    Err(format!("VA 0x{address:x} is outside all PE sections"))
}

fn load_sources(exe: &[u8]) -> Result<BTreeMap<&'static str, Vec<u8>>, String> {
    let mut sources = BTreeMap::new();
    for record in SOURCE_RECORDS {
        let offset = virtual_address_to_file_offset(exe, record.virtual_address)?;
        let encoded = exe
            .get(offset..offset + record.byte_count)
            .ok_or_else(|| format!("{} record exceeds the executable", record.name))?;
        let mut decoded = decode_embedded_shader_source(record.resource_hash, encoded)
            .map_err(|error| format!("{}: {error:?}", record.name))?;
        while decoded.last() == Some(&0) {
            decoded.pop();
        }
        sources.insert(record.name, decoded);
    }
    Ok(sources)
}

fn include_name(line: &[u8]) -> Option<&str> {
    let trimmed = line
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|index| &line[index..])?;
    if !trimmed.starts_with(b"#include") {
        return None;
    }
    let first = trimmed.iter().position(|byte| *byte == b'"')? + 1;
    let second = trimmed[first..].iter().position(|byte| *byte == b'"')? + first;
    std::str::from_utf8(&trimmed[first..second]).ok()
}

fn expand_source(
    name: &str,
    sources: &BTreeMap<&str, Vec<u8>>,
    included: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    if !included.insert(name.to_owned()) {
        return Ok(Vec::new());
    }
    if stack.iter().any(|entry| entry == name) {
        return Err(format!(
            "recursive include: {} -> {name}",
            stack.join(" -> ")
        ));
    }
    let source = sources
        .get(name)
        .ok_or_else(|| format!("unregistered include {name}"))?;
    stack.push(name.to_owned());
    let mut result = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let end = source[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(source.len(), |relative| start + relative + 1);
        let line = &source[start..end];
        if let Some(include) = include_name(line) {
            result.extend_from_slice(&expand_source(include, sources, included, stack)?);
            if !result.ends_with(b"\n") {
                result.extend_from_slice(b"\r\n");
            }
        } else {
            result.extend_from_slice(line);
        }
        start = end;
    }
    stack.pop();
    Ok(result)
}

fn simple_keys_from_collection(
    xml: &str,
) -> Result<Vec<[u8; CEYLON_SIMPLE_SHADER_KEY_LENGTH]>, String> {
    let mut inside_simple_group = false;
    let mut keys = Vec::new();
    for line in xml.lines().map(str::trim) {
        if line.starts_with("<SimpleShaderVSSimpleShaderPS_") {
            inside_simple_group = true;
            continue;
        }
        if line.starts_with("</SimpleShaderVSSimpleShaderPS_") {
            inside_simple_group = false;
            continue;
        }
        if !inside_simple_group || !line.starts_with('<') || !line.ends_with("/>") {
            continue;
        }
        let encoded = &line.as_bytes()[1..line.len() - 2];
        keys.push(
            encoded
                .try_into()
                .map_err(|_| format!("invalid Simple key element: {line}"))?,
        );
    }
    if keys.is_empty() {
        Err("no SimpleShaderVSSimpleShaderPS keys found".into())
    } else {
        Ok(keys)
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(window_class: *const WindowClass) -> u16;
        fn UnregisterClassW(class_name: *const u16, instance: *mut c_void) -> i32;
        fn CreateWindowExW(
            extended_style: u32,
            class_name: *const u16,
            window_name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: *mut c_void,
            menu: *mut c_void,
            instance: *mut c_void,
            parameter: *mut c_void,
        ) -> *mut c_void;
        fn DefWindowProcW(window: *mut c_void, message: u32, wparam: usize, lparam: isize)
        -> isize;
        fn DestroyWindow(window: *mut c_void) -> i32;
    }

    #[repr(C)]
    struct WindowClass {
        style: u32,
        window_proc: Option<unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize>,
        class_extra: i32,
        window_extra: i32,
        instance: *mut c_void,
        icon: *mut c_void,
        cursor: *mut c_void,
        background: *mut c_void,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    struct PresentParameters {
        back_buffer_width: u32,
        back_buffer_height: u32,
        back_buffer_format: u32,
        back_buffer_count: u32,
        multi_sample_type: u32,
        multi_sample_quality: u32,
        swap_effect: u32,
        device_window: *mut c_void,
        windowed: i32,
        enable_auto_depth_stencil: i32,
        auto_depth_stencil_format: u32,
        flags: u32,
        full_screen_refresh_rate_hz: u32,
        presentation_interval: u32,
    }

    #[repr(C)]
    struct ComObject {
        vtable: *const *const c_void,
    }

    type CgCreateContext = unsafe extern "C" fn() -> *mut c_void;
    type CgDestroyContext = unsafe extern "C" fn(*mut c_void);
    type CgCreateProgram = unsafe extern "C" fn(
        *mut c_void,
        i32,
        *const c_char,
        i32,
        *const c_char,
        *const *const c_char,
    ) -> *mut c_void;
    type CgDestroyProgram = unsafe extern "C" fn(*mut c_void);
    type CgGetProgramString = unsafe extern "C" fn(*mut c_void, i32) -> *const c_char;
    type CgGetLastListing = unsafe extern "C" fn(*mut c_void) -> *const c_char;
    type CgGetError = unsafe extern "C" fn() -> i32;
    type CgGetErrorString = unsafe extern "C" fn(i32) -> *const c_char;

    #[repr(C)]
    struct Blob {
        vtable: *const BlobVtable,
    }

    #[repr(C)]
    struct BlobVtable {
        query_interface:
            unsafe extern "system" fn(*mut Blob, *const c_void, *mut *mut c_void) -> i32,
        add_ref: unsafe extern "system" fn(*mut Blob) -> u32,
        release: unsafe extern "system" fn(*mut Blob) -> u32,
        get_buffer_pointer: unsafe extern "system" fn(*mut Blob) -> *mut c_void,
        get_buffer_size: unsafe extern "system" fn(*mut Blob) -> usize,
    }

    type D3dAssemble = unsafe extern "system" fn(
        *const c_void,
        usize,
        *const c_char,
        *const c_void,
        *mut c_void,
        u32,
        *mut *mut Blob,
        *mut *mut Blob,
    ) -> i32;

    type Direct3dCreate9 = unsafe extern "system" fn(u32) -> *mut ComObject;
    type CreateDevice = unsafe extern "system" fn(
        *mut ComObject,
        u32,
        u32,
        *mut c_void,
        u32,
        *mut PresentParameters,
        *mut *mut ComObject,
    ) -> i32;
    type CreateVertexShader =
        unsafe extern "system" fn(*mut ComObject, *const u32, *mut *mut ComObject) -> i32;
    type CreatePixelShader =
        unsafe extern "system" fn(*mut ComObject, *const u32, *mut *mut ComObject) -> i32;
    type Release = unsafe extern "system" fn(*mut ComObject) -> u32;

    struct Module(*mut c_void);

    impl Module {
        fn load(path: &Path) -> Result<Self, String> {
            let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
            wide.push(0);
            let module = unsafe { LoadLibraryW(wide.as_ptr()) };
            if module.is_null() {
                Err(format!("LoadLibraryW failed for {}", path.display()))
            } else {
                Ok(Self(module))
            }
        }

        unsafe fn proc(&self, name: &'static [u8]) -> Result<*mut c_void, String> {
            let proc = unsafe { GetProcAddress(self.0, name.as_ptr().cast()) };
            if proc.is_null() {
                Err(format!(
                    "GetProcAddress failed for {}",
                    String::from_utf8_lossy(&name[..name.len() - 1])
                ))
            } else {
                Ok(proc)
            }
        }
    }

    impl Drop for Module {
        fn drop(&mut self) {
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }

    unsafe extern "system" fn probe_window_proc(
        window: *mut c_void,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    unsafe fn com_method(object: *mut ComObject, index: usize) -> *const c_void {
        unsafe { *(*object).vtable.add(index) }
    }

    unsafe fn release_com(object: *mut ComObject) {
        if !object.is_null() {
            let release: Release = unsafe { std::mem::transmute(com_method(object, 2)) };
            unsafe { release(object) };
        }
    }

    pub struct D3d9Validator {
        _module: Module,
        class_name: Vec<u16>,
        instance: *mut c_void,
        window: *mut c_void,
        d3d: *mut ComObject,
        device: *mut ComObject,
        device_create_hresult: u32,
    }

    impl D3d9Validator {
        pub fn new() -> Result<Self, String> {
            let module = Module::load(Path::new("d3d9.dll"))?;
            let create_d3d: Direct3dCreate9 =
                unsafe { std::mem::transmute(module.proc(b"Direct3DCreate9\0")?) };
            let instance = unsafe { GetModuleHandleW(ptr::null()) };
            if instance.is_null() {
                return Err("GetModuleHandleW(null) failed".into());
            }
            let class_name: Vec<u16> = "SrdCgShaderProbeWindow"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let window_class = WindowClass {
                style: 0,
                window_proc: Some(probe_window_proc),
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: ptr::null_mut(),
                cursor: ptr::null_mut(),
                background: ptr::null_mut(),
                menu_name: ptr::null(),
                class_name: class_name.as_ptr(),
            };
            if unsafe { RegisterClassW(&window_class) } == 0 {
                return Err("RegisterClassW failed".into());
            }
            let window = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    class_name.as_ptr(),
                    0,
                    0,
                    0,
                    1,
                    1,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    instance,
                    ptr::null_mut(),
                )
            };
            if window.is_null() {
                unsafe { UnregisterClassW(class_name.as_ptr(), instance) };
                return Err("CreateWindowExW failed".into());
            }
            let d3d = unsafe { create_d3d(32) };
            if d3d.is_null() {
                unsafe {
                    DestroyWindow(window);
                    UnregisterClassW(class_name.as_ptr(), instance);
                }
                return Err("Direct3DCreate9 returned null".into());
            }
            let mut parameters = PresentParameters {
                back_buffer_width: 1,
                back_buffer_height: 1,
                back_buffer_format: 0,
                back_buffer_count: 1,
                multi_sample_type: 0,
                multi_sample_quality: 0,
                swap_effect: 1,
                device_window: window,
                windowed: 1,
                enable_auto_depth_stencil: 0,
                auto_depth_stencil_format: 0,
                flags: 0,
                full_screen_refresh_rate_hz: 0,
                presentation_interval: 0,
            };
            let create_device: CreateDevice = unsafe { std::mem::transmute(com_method(d3d, 16)) };
            let mut device = ptr::null_mut();
            let hr =
                unsafe { create_device(d3d, 0, 1, window, 0x20, &mut parameters, &mut device) };
            if hr < 0 || device.is_null() {
                unsafe {
                    release_com(d3d);
                    DestroyWindow(window);
                    UnregisterClassW(class_name.as_ptr(), instance);
                }
                return Err(format!(
                    "IDirect3D9::CreateDevice(HAL, software VP) HRESULT 0x{:08x}",
                    hr as u32
                ));
            }
            Ok(Self {
                _module: module,
                class_name,
                instance,
                window,
                d3d,
                device,
                device_create_hresult: hr as u32,
            })
        }

        pub fn device_create_hresult(&self) -> u32 {
            self.device_create_hresult
        }

        pub fn validate(&self, stage: &str, bytecode: &[u8]) -> Result<u32, String> {
            if bytecode.len() % 4 != 0 {
                return Err(format!(
                    "{stage} bytecode length {} is not DWORD-aligned",
                    bytecode.len()
                ));
            }
            let tokens: Vec<u32> = bytecode
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
                .collect();
            let (method_index, expected_token) = match stage {
                "vs" => (91, 0xfffe_0300),
                "ps" => (106, 0xffff_0300),
                _ => return Err(format!("unknown shader stage {stage}")),
            };
            if tokens.first().copied() != Some(expected_token) {
                return Err(format!("{stage} bytecode has the wrong profile token"));
            }
            let mut shader = ptr::null_mut();
            let hr = unsafe {
                if stage == "vs" {
                    let create: CreateVertexShader =
                        std::mem::transmute(com_method(self.device, method_index));
                    create(self.device, tokens.as_ptr(), &mut shader)
                } else {
                    let create: CreatePixelShader =
                        std::mem::transmute(com_method(self.device, method_index));
                    create(self.device, tokens.as_ptr(), &mut shader)
                }
            };
            if hr < 0 || shader.is_null() {
                return Err(format!(
                    "IDirect3DDevice9::Create{}Shader HRESULT 0x{:08x}",
                    if stage == "vs" { "Vertex" } else { "Pixel" },
                    hr as u32
                ));
            }
            unsafe { release_com(shader) };
            Ok(hr as u32)
        }
    }

    impl Drop for D3d9Validator {
        fn drop(&mut self) {
            unsafe {
                release_com(self.device);
                release_com(self.d3d);
                DestroyWindow(self.window);
                UnregisterClassW(self.class_name.as_ptr(), self.instance);
            }
        }
    }

    fn c_string(pointer: *const c_char) -> String {
        if pointer.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned()
        }
    }

    unsafe fn release_blob(blob: *mut Blob) {
        if !blob.is_null() {
            unsafe { ((*(*blob).vtable).release)(blob) };
        }
    }

    pub fn compile(
        cg_path: &Path,
        source: &[u8],
        profile: i32,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        let cg_dir = cg_path
            .parent()
            .ok_or_else(|| "cg.dll path has no parent".to_owned())?;
        let mut wide_dir: Vec<u16> = cg_dir.as_os_str().encode_wide().collect();
        wide_dir.push(0);
        unsafe {
            SetDllDirectoryW(wide_dir.as_ptr());
        }
        let cg = Module::load(cg_path)?;
        let d3d = Module::load(Path::new("d3dcompiler_47.dll"))?;

        unsafe {
            let create_context: CgCreateContext =
                std::mem::transmute(cg.proc(b"cgCreateContext\0")?);
            let destroy_context: CgDestroyContext =
                std::mem::transmute(cg.proc(b"cgDestroyContext\0")?);
            let create_program: CgCreateProgram =
                std::mem::transmute(cg.proc(b"cgCreateProgram\0")?);
            let destroy_program: CgDestroyProgram =
                std::mem::transmute(cg.proc(b"cgDestroyProgram\0")?);
            let get_program_string: CgGetProgramString =
                std::mem::transmute(cg.proc(b"cgGetProgramString\0")?);
            let get_last_listing: CgGetLastListing =
                std::mem::transmute(cg.proc(b"cgGetLastListing\0")?);
            let get_error: CgGetError = std::mem::transmute(cg.proc(b"cgGetError\0")?);
            let get_error_string: CgGetErrorString =
                std::mem::transmute(cg.proc(b"cgGetErrorString\0")?);
            let assemble: D3dAssemble = std::mem::transmute(d3d.proc(b"D3DAssemble\0")?);

            let context = create_context();
            if context.is_null() {
                return Err("cgCreateContext returned null".into());
            }
            let mut terminated = source.to_vec();
            terminated.push(0);
            let program = create_program(
                context,
                0x1010,
                terminated.as_ptr().cast(),
                profile,
                c"main".as_ptr(),
                ptr::null(),
            );
            if program.is_null() {
                let error = get_error();
                let message = c_string(get_error_string(error));
                let listing = c_string(get_last_listing(context));
                destroy_context(context);
                return Err(format!("Cg error {error}: {message}\n{listing}"));
            }
            let assembly_pointer = get_program_string(program, 0x100a);
            if assembly_pointer.is_null() {
                destroy_program(program);
                destroy_context(context);
                return Err("cgGetProgramString(CG_COMPILED_PROGRAM) returned null".into());
            }
            let assembly = CStr::from_ptr(assembly_pointer).to_bytes().to_vec();

            let mut bytecode_blob: *mut Blob = ptr::null_mut();
            let mut error_blob: *mut Blob = ptr::null_mut();
            let hr = assemble(
                assembly.as_ptr().cast(),
                assembly.len(),
                c"cg_probe.asm".as_ptr(),
                ptr::null(),
                ptr::null_mut(),
                0,
                &mut bytecode_blob,
                &mut error_blob,
            );
            let error_text = if error_blob.is_null() {
                String::new()
            } else {
                let pointer = ((*(*error_blob).vtable).get_buffer_pointer)(error_blob);
                let size = ((*(*error_blob).vtable).get_buffer_size)(error_blob);
                String::from_utf8_lossy(std::slice::from_raw_parts(pointer.cast(), size))
                    .into_owned()
            };
            release_blob(error_blob);
            if hr < 0 || bytecode_blob.is_null() {
                destroy_program(program);
                destroy_context(context);
                return Err(format!(
                    "D3DAssemble HRESULT 0x{:08x}\n{error_text}",
                    hr as u32
                ));
            }
            let bytecode_pointer = ((*(*bytecode_blob).vtable).get_buffer_pointer)(bytecode_blob);
            let bytecode_size = ((*(*bytecode_blob).vtable).get_buffer_size)(bytecode_blob);
            let bytecode =
                std::slice::from_raw_parts(bytecode_pointer.cast(), bytecode_size).to_vec();
            release_blob(bytecode_blob);
            destroy_program(program);
            destroy_context(context);
            Ok((assembly, bytecode))
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod win {
    use super::*;

    pub fn compile(
        _cg_path: &Path,
        _source: &[u8],
        _profile: i32,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        Err("cg_shader_probe requires Windows".into())
    }

    pub struct D3d9Validator;

    impl D3d9Validator {
        pub fn new() -> Result<Self, String> {
            Err("cg_shader_probe requires Windows".into())
        }

        pub fn device_create_hresult(&self) -> u32 {
            unreachable!()
        }

        pub fn validate(&self, _stage: &str, _bytecode: &[u8]) -> Result<u32, String> {
            Err("cg_shader_probe requires Windows".into())
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err(format!(
            "usage: {} <chusanApp.exe> <cg.dll> <18-byte-key|shadercollect.xml> <output-dir>",
            args.first().map_or("cg-shader-probe", String::as_str)
        ));
    }
    let exe_path = PathBuf::from(&args[1]);
    let cg_path = PathBuf::from(&args[2]);
    let key_argument = Path::new(&args[3]);
    let keys = if key_argument.is_file() {
        let xml = fs::read_to_string(key_argument)
            .map_err(|error| format!("{}: {error}", key_argument.display()))?;
        simple_keys_from_collection(&xml)?
    } else {
        vec![args[3].as_bytes().try_into().map_err(|_| {
            format!("Simple shader key must be exactly {CEYLON_SIMPLE_SHADER_KEY_LENGTH} bytes")
        })?]
    };
    let output_dir = PathBuf::from(&args[4]);

    let exe = fs::read(&exe_path).map_err(|error| format!("{}: {error}", exe_path.display()))?;
    let sources = load_sources(&exe)?;
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("{}: {error}", output_dir.display()))?;

    let validator = win::D3d9Validator::new()?;
    let mut manifest = String::from(
        "key\tstage\tassembly_bytes\tbytecode_bytes\tdevice_create_hresult\tshader_create_hresult\n",
    );
    for key in keys {
        let key_text = std::str::from_utf8(&key).map_err(|error| error.to_string())?;
        let bits = CeylonSimpleShaderBits::from_compact_key(key);
        let prefix = bits.define_prefix();
        for (stage_name, root, profile) in [
            ("vs", "SimpleShaderVS.cg", 6157),
            ("ps", "SimpleShaderPS.cg", 6165),
        ] {
            let mut included = BTreeSet::new();
            let mut source = prefix.as_bytes().to_vec();
            source.push(b'\n');
            source.extend_from_slice(&expand_source(
                root,
                &sources,
                &mut included,
                &mut Vec::new(),
            )?);
            let (assembly, bytecode) = win::compile(&cg_path, &source, profile)
                .map_err(|error| format!("{key_text} {stage_name}: {error}"))?;
            let shader_create_hresult = validator
                .validate(stage_name, &bytecode)
                .map_err(|error| format!("{key_text} {stage_name}: {error}"))?;
            manifest.push_str(&format!(
                "{key_text}\t{stage_name}\t{}\t{}\t0x{:08X}\t0x{shader_create_hresult:08X}\n",
                assembly.len(),
                bytecode.len(),
                validator.device_create_hresult(),
            ));
            let stem = format!("SimpleShader{stage_name}_{key_text}");
            fs::write(output_dir.join(format!("{stem}.asm")), assembly)
                .map_err(|error| error.to_string())?;
            fs::write(output_dir.join(format!("{stem}.bin")), bytecode)
                .map_err(|error| error.to_string())?;
            fs::write(output_dir.join(format!("{stem}.cg")), source)
                .map_err(|error| error.to_string())?;
        }
    }
    fs::write(output_dir.join("manifest.tsv"), manifest).map_err(|error| error.to_string())?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
