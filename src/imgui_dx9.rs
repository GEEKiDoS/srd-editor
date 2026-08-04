use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::slice;

use imgui::internal::RawWrapper;
use imgui::{BackendFlags, Context, DrawCmd, DrawData, DrawIdx, TextureId, Textures};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D9::{
    D3DBLEND_INVSRCALPHA, D3DBLEND_ONE, D3DBLEND_SRCALPHA, D3DBLENDOP_ADD, D3DCULL_NONE,
    D3DFILL_SOLID, D3DFMT_A8R8G8B8, D3DFMT_INDEX16, D3DFMT_INDEX32, D3DFVF_DIFFUSE, D3DFVF_TEX1,
    D3DFVF_XYZ, D3DLOCK_DISCARD, D3DLOCKED_RECT, D3DPOOL_DEFAULT, D3DPT_TRIANGLELIST,
    D3DRS_ALPHABLENDENABLE, D3DRS_ALPHATESTENABLE, D3DRS_BLENDOP, D3DRS_CLIPPING, D3DRS_CULLMODE,
    D3DRS_DESTBLEND, D3DRS_DESTBLENDALPHA, D3DRS_FILLMODE, D3DRS_FOGENABLE, D3DRS_LIGHTING,
    D3DRS_RANGEFOGENABLE, D3DRS_SCISSORTESTENABLE, D3DRS_SEPARATEALPHABLENDENABLE, D3DRS_SHADEMODE,
    D3DRS_SPECULARENABLE, D3DRS_SRCBLEND, D3DRS_SRCBLENDALPHA, D3DRS_STENCILENABLE, D3DRS_ZENABLE,
    D3DRS_ZWRITEENABLE, D3DSAMP_MAGFILTER, D3DSAMP_MINFILTER, D3DSBT_ALL, D3DSHADE_GOURAUD,
    D3DTA_DIFFUSE, D3DTA_TEXTURE, D3DTEXF_LINEAR, D3DTOP_DISABLE, D3DTOP_MODULATE,
    D3DTRANSFORMSTATETYPE, D3DTS_PROJECTION, D3DTS_VIEW, D3DTSS_ALPHAARG1, D3DTSS_ALPHAARG2,
    D3DTSS_ALPHAOP, D3DTSS_COLORARG1, D3DTSS_COLORARG2, D3DTSS_COLOROP, D3DUSAGE_DYNAMIC,
    D3DUSAGE_WRITEONLY, D3DVIEWPORT9, IDirect3DBaseTexture9, IDirect3DDevice9,
    IDirect3DIndexBuffer9, IDirect3DStateBlock9, IDirect3DTexture9, IDirect3DVertexBuffer9,
};
use windows::core::{Error, HRESULT, Interface, Result};
use windows_numerics::Matrix4x4;

const E_FAIL: HRESULT = HRESULT(0x8000_4005_u32 as i32);
const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057_u32 as i32);
const FONT_TEXTURE_ID: usize = usize::MAX;
const VERTEX_GROWTH: usize = 5_000;
const INDEX_GROWTH: usize = 10_000;
const D3DTS_WORLD: D3DTRANSFORMSTATETYPE = D3DTRANSFORMSTATETYPE(256);
const D3DFVF_IMGUI_VERTEX: u32 = D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX1;

const IDENTITY: Matrix4x4 = Matrix4x4 {
    M11: 1.0,
    M12: 0.0,
    M13: 0.0,
    M14: 0.0,
    M21: 0.0,
    M22: 1.0,
    M23: 0.0,
    M24: 0.0,
    M31: 0.0,
    M32: 0.0,
    M33: 1.0,
    M34: 0.0,
    M41: 0.0,
    M42: 0.0,
    M43: 0.0,
    M44: 1.0,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct ImguiVertex {
    position: [f32; 3],
    color_bgra: [u8; 4],
    uv: [f32; 2],
}

/// Dear ImGui renderer backend following the official
/// `backends/imgui_impl_dx9.cpp` state, buffer and half-pixel behavior.
/// It uses the D3D9 fixed-function pipeline and therefore needs no D3DX or
/// shader compiler.
pub struct ImguiDx9Renderer {
    device: IDirect3DDevice9,
    font_texture: Option<IDirect3DBaseTexture9>,
    vertex_buffer: Option<(IDirect3DVertexBuffer9, usize)>,
    index_buffer: Option<(IDirect3DIndexBuffer9, usize)>,
    textures: Textures<IDirect3DBaseTexture9>,
}

impl ImguiDx9Renderer {
    pub fn new(context: &mut Context, device: &IDirect3DDevice9) -> Result<Self> {
        context.io_mut().backend_flags |= BackendFlags::RENDERER_HAS_VTX_OFFSET;
        context.set_renderer_name(String::from("srd-editor-imgui-dx9"));
        let mut renderer = Self {
            device: device.clone(),
            font_texture: None,
            vertex_buffer: None,
            index_buffer: None,
            textures: Textures::new(),
        };
        renderer.create_device_objects(context)?;
        Ok(renderer)
    }

    pub fn textures(&self) -> &Textures<IDirect3DBaseTexture9> {
        &self.textures
    }

    pub fn textures_mut(&mut self) -> &mut Textures<IDirect3DBaseTexture9> {
        &mut self.textures
    }

    pub fn invalidate_device_objects(&mut self, context: &mut Context) {
        self.vertex_buffer = None;
        self.index_buffer = None;
        self.font_texture = None;
        context.fonts().tex_id = TextureId::new(0);
    }

    pub fn create_device_objects(&mut self, context: &mut Context) -> Result<()> {
        self.create_font_texture(context)
    }

    pub fn render(&mut self, draw_data: &DrawData) -> Result<()> {
        if draw_data.display_size[0] <= 0.0 || draw_data.display_size[1] <= 0.0 {
            return Ok(());
        }
        self.ensure_buffers(draw_data)?;
        let _state_backup = StateBackup::capture(&self.device)?;
        self.write_buffers(draw_data)?;
        self.setup_render_state(draw_data)?;
        self.draw_command_lists(draw_data)
    }

    fn ensure_buffers(&mut self, draw_data: &DrawData) -> Result<()> {
        let vertex_count = draw_data.total_vtx_count.max(0) as usize;
        let index_count = draw_data.total_idx_count.max(0) as usize;
        if self
            .vertex_buffer
            .as_ref()
            .is_none_or(|(_, capacity)| *capacity < vertex_count)
        {
            self.vertex_buffer = Some(create_vertex_buffer(&self.device, vertex_count)?);
        }
        if self
            .index_buffer
            .as_ref()
            .is_none_or(|(_, capacity)| *capacity < index_count)
        {
            self.index_buffer = Some(create_index_buffer(&self.device, index_count)?);
        }
        Ok(())
    }

    fn write_buffers(&mut self, draw_data: &DrawData) -> Result<()> {
        let vertex_count = draw_data.total_vtx_count.max(0) as usize;
        let index_count = draw_data.total_idx_count.max(0) as usize;
        let (vertex_buffer, _) = self
            .vertex_buffer
            .as_mut()
            .ok_or_else(|| Error::new(E_FAIL, "Dear ImGui DX9 vertex buffer was not created"))?;
        let (index_buffer, _) = self
            .index_buffer
            .as_mut()
            .ok_or_else(|| Error::new(E_FAIL, "Dear ImGui DX9 index buffer was not created"))?;

        let mut vertex_ptr = ptr::null_mut::<ImguiVertex>();
        let mut index_ptr = ptr::null_mut::<DrawIdx>();
        unsafe {
            vertex_buffer.Lock(
                0,
                (vertex_count * mem::size_of::<ImguiVertex>()) as u32,
                &mut vertex_ptr as *mut _ as *mut *mut c_void,
                D3DLOCK_DISCARD as u32,
            )?;
        }
        if let Err(error) = unsafe {
            index_buffer.Lock(
                0,
                (index_count * mem::size_of::<DrawIdx>()) as u32,
                &mut index_ptr as *mut _ as *mut *mut c_void,
                D3DLOCK_DISCARD as u32,
            )
        } {
            unsafe { vertex_buffer.Unlock()? };
            return Err(error);
        }

        let mut vertices = unsafe { slice::from_raw_parts_mut(vertex_ptr, vertex_count) };
        let mut indices = unsafe { slice::from_raw_parts_mut(index_ptr, index_count) };
        for draw_list in draw_data.draw_lists() {
            let source_vertices = draw_list.vtx_buffer();
            for (source, destination) in source_vertices.iter().zip(vertices.iter_mut()) {
                *destination = ImguiVertex {
                    position: [source.pos[0], source.pos[1], 0.0],
                    color_bgra: [source.col[2], source.col[1], source.col[0], source.col[3]],
                    uv: source.uv,
                };
            }
            let source_indices = draw_list.idx_buffer();
            indices[..source_indices.len()].copy_from_slice(source_indices);
            vertices = &mut vertices[source_vertices.len()..];
            indices = &mut indices[source_indices.len()..];
        }

        unsafe {
            vertex_buffer.Unlock()?;
            index_buffer.Unlock()?;
            self.device.SetStreamSource(
                0,
                &*vertex_buffer,
                0,
                mem::size_of::<ImguiVertex>() as u32,
            )?;
            self.device.SetIndices(&*index_buffer)?;
            self.device.SetFVF(D3DFVF_IMGUI_VERTEX)?;
        }
        Ok(())
    }

    fn setup_render_state(&self, draw_data: &DrawData) -> Result<()> {
        let framebuffer_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let framebuffer_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        let viewport = D3DVIEWPORT9 {
            X: 0,
            Y: 0,
            Width: framebuffer_width.max(0.0) as u32,
            Height: framebuffer_height.max(0.0) as u32,
            MinZ: 0.0,
            MaxZ: 1.0,
        };
        let left = draw_data.display_pos[0] + 0.5;
        let right = draw_data.display_pos[0] + draw_data.display_size[0] + 0.5;
        let top = draw_data.display_pos[1] + 0.5;
        let bottom = draw_data.display_pos[1] + draw_data.display_size[1] + 0.5;
        let projection = Matrix4x4 {
            M11: 2.0 / (right - left),
            M12: 0.0,
            M13: 0.0,
            M14: 0.0,
            M21: 0.0,
            M22: 2.0 / (top - bottom),
            M23: 0.0,
            M24: 0.0,
            M31: 0.0,
            M32: 0.0,
            M33: 0.5,
            M34: 0.0,
            M41: (left + right) / (left - right),
            M42: (top + bottom) / (bottom - top),
            M43: 0.5,
            M44: 1.0,
        };

        unsafe {
            self.device.SetViewport(&viewport)?;
            self.device.SetPixelShader(None)?;
            self.device.SetVertexShader(None)?;
            self.device
                .SetRenderState(D3DRS_FILLMODE, D3DFILL_SOLID.0 as u32)?;
            self.device
                .SetRenderState(D3DRS_SHADEMODE, D3DSHADE_GOURAUD.0 as u32)?;
            self.device.SetRenderState(D3DRS_ZWRITEENABLE, 0)?;
            self.device.SetRenderState(D3DRS_ALPHATESTENABLE, 0)?;
            self.device
                .SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE.0 as u32)?;
            self.device.SetRenderState(D3DRS_ZENABLE, 0)?;
            self.device.SetRenderState(D3DRS_ALPHABLENDENABLE, 1)?;
            self.device
                .SetRenderState(D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as u32)?;
            self.device
                .SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32)?;
            self.device
                .SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32)?;
            self.device
                .SetRenderState(D3DRS_SEPARATEALPHABLENDENABLE, 1)?;
            self.device
                .SetRenderState(D3DRS_SRCBLENDALPHA, D3DBLEND_ONE.0 as u32)?;
            self.device
                .SetRenderState(D3DRS_DESTBLENDALPHA, D3DBLEND_INVSRCALPHA.0 as u32)?;
            self.device.SetRenderState(D3DRS_SCISSORTESTENABLE, 1)?;
            self.device.SetRenderState(D3DRS_FOGENABLE, 0)?;
            self.device.SetRenderState(D3DRS_RANGEFOGENABLE, 0)?;
            self.device.SetRenderState(D3DRS_SPECULARENABLE, 0)?;
            self.device.SetRenderState(D3DRS_STENCILENABLE, 0)?;
            self.device.SetRenderState(D3DRS_CLIPPING, 1)?;
            self.device.SetRenderState(D3DRS_LIGHTING, 0)?;
            self.device
                .SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE.0 as u32)?;
            self.device
                .SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE)?;
            self.device
                .SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE)?;
            self.device
                .SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as u32)?;
            self.device
                .SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE)?;
            self.device
                .SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE)?;
            self.device
                .SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as u32)?;
            self.device
                .SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_DISABLE.0 as u32)?;
            self.device
                .SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32)?;
            self.device
                .SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32)?;
            self.device.SetTransform(D3DTS_WORLD, &IDENTITY)?;
            self.device.SetTransform(D3DTS_VIEW, &IDENTITY)?;
            self.device.SetTransform(D3DTS_PROJECTION, &projection)?;
        }
        Ok(())
    }

    fn draw_command_lists(&mut self, draw_data: &DrawData) -> Result<()> {
        let clip_offset = draw_data.display_pos;
        let clip_scale = draw_data.framebuffer_scale;
        let mut global_vertex_offset = 0usize;
        let mut global_index_offset = 0usize;
        for draw_list in draw_data.draw_lists() {
            for command in draw_list.commands() {
                match command {
                    DrawCmd::Elements { count, cmd_params } => {
                        let clip_min_x = (cmd_params.clip_rect[0] - clip_offset[0]) * clip_scale[0];
                        let clip_min_y = (cmd_params.clip_rect[1] - clip_offset[1]) * clip_scale[1];
                        let clip_max_x = (cmd_params.clip_rect[2] - clip_offset[0]) * clip_scale[0];
                        let clip_max_y = (cmd_params.clip_rect[3] - clip_offset[1]) * clip_scale[1];
                        if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                            continue;
                        }
                        let rectangle = RECT {
                            left: clip_min_x as i32,
                            top: clip_min_y as i32,
                            right: clip_max_x as i32,
                            bottom: clip_max_y as i32,
                        };
                        let texture = if cmd_params.texture_id.id() == FONT_TEXTURE_ID {
                            self.font_texture.as_ref()
                        } else {
                            self.textures.get(cmd_params.texture_id)
                        }
                        .ok_or_else(|| {
                            Error::new(
                                E_INVALIDARG,
                                format!(
                                    "Dear ImGui referenced unknown texture {}",
                                    cmd_params.texture_id.id()
                                ),
                            )
                        })?;
                        unsafe {
                            self.device.SetTexture(0, texture)?;
                            self.device.SetScissorRect(&rectangle)?;
                            self.device.DrawIndexedPrimitive(
                                D3DPT_TRIANGLELIST,
                                (global_vertex_offset + cmd_params.vtx_offset) as i32,
                                0,
                                draw_list.vtx_buffer().len() as u32,
                                (global_index_offset + cmd_params.idx_offset) as u32,
                                (count / 3) as u32,
                            )?;
                        }
                    }
                    DrawCmd::ResetRenderState => self.setup_render_state(draw_data)?,
                    DrawCmd::RawCallback { callback, raw_cmd } => unsafe {
                        callback(draw_list.raw(), raw_cmd)
                    },
                }
            }
            global_vertex_offset += draw_list.vtx_buffer().len();
            global_index_offset += draw_list.idx_buffer().len();
        }
        Ok(())
    }

    fn create_font_texture(&mut self, context: &mut Context) -> Result<()> {
        let fonts = context.fonts();
        let atlas = fonts.build_rgba32_texture();
        let mut texture = None;
        unsafe {
            self.device.CreateTexture(
                atlas.width,
                atlas.height,
                1,
                D3DUSAGE_DYNAMIC as u32,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut texture,
                ptr::null_mut(),
            )?;
        }
        let texture: IDirect3DTexture9 = texture
            .ok_or_else(|| Error::new(E_FAIL, "CreateTexture returned a null font texture"))?;
        let mut locked = D3DLOCKED_RECT::default();
        unsafe { texture.LockRect(0, &mut locked, ptr::null(), 0)? };
        for y in 0..atlas.height as usize {
            let source = &atlas.data[y * atlas.width as usize * 4..][..atlas.width as usize * 4];
            let destination = unsafe {
                slice::from_raw_parts_mut(
                    (locked.pBits as *mut u8).add(y * locked.Pitch as usize),
                    atlas.width as usize * 4,
                )
            };
            for (source, destination) in source.chunks_exact(4).zip(destination.chunks_exact_mut(4))
            {
                destination.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
            }
        }
        unsafe { texture.UnlockRect(0)? };
        self.font_texture = Some(texture.cast()?);
        fonts.tex_id = TextureId::new(FONT_TEXTURE_ID);
        Ok(())
    }
}

fn create_vertex_buffer(
    device: &IDirect3DDevice9,
    requested: usize,
) -> Result<(IDirect3DVertexBuffer9, usize)> {
    let capacity = requested.saturating_add(VERTEX_GROWTH);
    let mut buffer = None;
    unsafe {
        device.CreateVertexBuffer(
            (capacity * mem::size_of::<ImguiVertex>()) as u32,
            (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as u32,
            D3DFVF_IMGUI_VERTEX,
            D3DPOOL_DEFAULT,
            &mut buffer,
            ptr::null_mut(),
        )?;
    }
    Ok((
        buffer.ok_or_else(|| Error::new(E_FAIL, "CreateVertexBuffer returned null"))?,
        capacity,
    ))
}

fn create_index_buffer(
    device: &IDirect3DDevice9,
    requested: usize,
) -> Result<(IDirect3DIndexBuffer9, usize)> {
    let capacity = requested.saturating_add(INDEX_GROWTH);
    let format = if mem::size_of::<DrawIdx>() == 2 {
        D3DFMT_INDEX16
    } else {
        D3DFMT_INDEX32
    };
    let mut buffer = None;
    unsafe {
        device.CreateIndexBuffer(
            (capacity * mem::size_of::<DrawIdx>()) as u32,
            (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as u32,
            format,
            D3DPOOL_DEFAULT,
            &mut buffer,
            ptr::null_mut(),
        )?;
    }
    Ok((
        buffer.ok_or_else(|| Error::new(E_FAIL, "CreateIndexBuffer returned null"))?,
        capacity,
    ))
}

struct StateBackup {
    device: IDirect3DDevice9,
    block: IDirect3DStateBlock9,
    world: Matrix4x4,
    view: Matrix4x4,
    projection: Matrix4x4,
}

impl StateBackup {
    fn capture(device: &IDirect3DDevice9) -> Result<Self> {
        unsafe {
            let block = device.CreateStateBlock(D3DSBT_ALL)?;
            block.Capture()?;
            let mut world = Matrix4x4::default();
            let mut view = Matrix4x4::default();
            let mut projection = Matrix4x4::default();
            device.GetTransform(D3DTS_WORLD, &mut world)?;
            device.GetTransform(D3DTS_VIEW, &mut view)?;
            device.GetTransform(D3DTS_PROJECTION, &mut projection)?;
            Ok(Self {
                device: device.clone(),
                block,
                world,
                view,
                projection,
            })
        }
    }
}

impl Drop for StateBackup {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.SetTransform(D3DTS_WORLD, &self.world);
            let _ = self.device.SetTransform(D3DTS_VIEW, &self.view);
            let _ = self.device.SetTransform(D3DTS_PROJECTION, &self.projection);
            let _ = self.block.Apply();
        }
    }
}
