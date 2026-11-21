//! Minimal `zwlr_screencopy_manager_v1` for xdg-desktop-portal-wlr (W449).
//!
//! SHM Xrgb8888 only. Frames queue until the render loop calls
//! [`ScreencopyState::drain_for_output`] and [`PendingScreencopy::submit_pixels`].

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use smithay::output::{Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1::{self, Flags, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
};
use smithay::reexports::wayland_server::protocol::{wl_buffer::WlBuffer, wl_shm::Format as ShmFormat};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Physical, Point, Rectangle, Size};
use smithay::wayland::shm;
use tracing::{trace, warn};

use crate::state::{AnvilState, Backend};

const VERSION: u32 = 3;

fn stride_for(size: Size<i32, Physical>) -> i32 {
    size.w.saturating_mul(4)
}

fn dummy_output() -> Output {
    Output::new(
        "mde-invalid".into(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: String::new(),
            model: String::new(),
        },
    )
}

#[derive(Debug, Default)]
pub struct ScreencopyState {
    pending: VecDeque<PendingScreencopy>,
}

impl ScreencopyState {
    pub fn new<D>(dh: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ZwlrScreencopyManagerV1, ()> + 'static,
    {
        dh.create_global::<D, ZwlrScreencopyManagerV1, ()>(VERSION, ());
        Self::default()
    }

    pub fn push(&mut self, frame: PendingScreencopy) {
        self.pending.push_back(frame);
    }

    pub fn drain_for_output(&mut self, output: &Output) -> Vec<PendingScreencopy> {
        let mut out = Vec::new();
        let mut rest = VecDeque::new();
        while let Some(frame) = self.pending.pop_front() {
            if frame.output == *output && frame.buffer.is_some() {
                out.push(frame);
            } else {
                rest.push_back(frame);
            }
        }
        self.pending = rest;
        out
    }

    pub fn has_pending_for(&self, output: &Output) -> bool {
        self.pending
            .iter()
            .any(|f| &f.output == output && f.buffer.is_some())
    }
}

#[derive(Debug)]
pub struct PendingScreencopy {
    pub frame: ZwlrScreencopyFrameV1,
    pub output: Output,
    pub buffer_size: Size<i32, Physical>,
    pub region_loc: Point<i32, Physical>,
    pub overlay_cursor: bool,
    pub with_damage: bool,
    pub buffer: Option<WlBuffer>,
    submitted: bool,
}

impl Drop for PendingScreencopy {
    fn drop(&mut self) {
        if !self.submitted {
            self.frame.failed();
        }
    }
}

impl PendingScreencopy {
    pub fn submit_pixels(&mut self, pixels: &[u8], y_invert: bool) -> bool {
        let Some(buffer) = self.buffer.as_ref() else {
            return false;
        };
        let stride = stride_for(self.buffer_size);
        let need = (self.buffer_size.h as usize).saturating_mul(stride as usize);
        if pixels.len() < need {
            warn!(got = pixels.len(), need, "screencopy pixel buffer too small");
            self.fail();
            return false;
        }
        let ok = shm::with_buffer_contents_mut(buffer, |ptr, len, data| {
            if data.format != ShmFormat::Xrgb8888
                || data.width != self.buffer_size.w
                || data.height != self.buffer_size.h
                || data.stride != stride
            {
                return false;
            }
            if len < need {
                return false;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr, need);
            }
            true
        })
        .unwrap_or(false);

        if !ok {
            self.fail();
            return false;
        }

        self.frame.flags(if y_invert {
            Flags::YInvert
        } else {
            Flags::empty()
        });
        if self.with_damage {
            self.frame.damage(
                0,
                0,
                self.buffer_size.w as u32,
                self.buffer_size.h as u32,
            );
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let tv_sec = now.as_secs();
        self.frame.ready(
            (tv_sec >> 32) as u32,
            (tv_sec & 0xffff_ffff) as u32,
            now.subsec_nanos(),
        );
        self.submitted = true;
        true
    }

    pub fn fail(&mut self) {
        if !self.submitted {
            self.frame.failed();
            self.submitted = true;
        }
    }
}

pub fn output_buffer_size(output: &Output) -> Option<Size<i32, Physical>> {
    Some(output.current_mode()?.size)
}

/// Pack tightly-strided Xrgb rows from a possibly larger `src_stride`.
/// Read an Xrgb8888 framebuffer into a tightly packed CPU buffer.
pub fn copy_fb_xrgb<R>(
    renderer: &mut R,
    fb: &R::Framebuffer<'_>,
    size: Size<i32, Physical>,
) -> Option<(Vec<u8>, bool)>
where
    R: smithay::backend::renderer::ExportMem,
{
    use smithay::backend::allocator::Fourcc;
    use smithay::backend::renderer::{ExportMem, TextureMapping};
    use smithay::utils::{Buffer as BufferCoords, Rectangle};

    let region = Rectangle::<i32, BufferCoords>::from_size((size.w, size.h).into());
    let mapping = renderer
        .copy_framebuffer(fb, region, Fourcc::Xrgb8888)
        .ok()?;
    let y_invert = mapping.flipped();
    let slice = ExportMem::map_texture(renderer, &mapping).ok()?;
    let stride = size.w.saturating_mul(4) as usize;
    let need = stride.saturating_mul(size.h as usize);
    if slice.len() == need {
        return Some((slice.to_vec(), y_invert));
    }
    // Some GLES paths return stride = width*4 with no padding; otherwise pack.
    let src_stride = if size.h > 0 {
        slice.len() / size.h as usize
    } else {
        stride
    };
    pack_xrgb(slice, src_stride.max(stride), size).map(|v| (v, y_invert))
}

pub fn pack_xrgb(src: &[u8], src_stride: usize, size: Size<i32, Physical>) -> Option<Vec<u8>> {
    let w = size.w as usize;
    let h = size.h as usize;
    let dst_stride = w.saturating_mul(4);
    if src_stride < dst_stride {
        return None;
    }
    let min_len = src_stride
        .saturating_mul(h.saturating_sub(1))
        .saturating_add(dst_stride);
    if src.len() < min_len {
        return None;
    }
    let mut out = vec![0u8; dst_stride.saturating_mul(h)];
    for y in 0..h {
        let s = y * src_stride;
        let d = y * dst_stride;
        out[d..d + dst_stride].copy_from_slice(&src[s..s + dst_stride]);
    }
    Some(out)
}

#[derive(Debug)]
pub struct FrameData {
    output: Output,
    buffer_size: Size<i32, Physical>,
    region_loc: Point<i32, Physical>,
    overlay_cursor: bool,
    copied: Arc<AtomicBool>,
}

fn announce_buffers(frame: &ZwlrScreencopyFrameV1, buffer_size: Size<i32, Physical>) {
    frame.buffer(
        ShmFormat::Xrgb8888,
        buffer_size.w as u32,
        buffer_size.h as u32,
        stride_for(buffer_size) as u32,
    );
    if frame.version() >= 3 {
        frame.buffer_done();
    }
}

impl<BackendData: Backend + 'static> GlobalDispatch<ZwlrScreencopyManagerV1, ()>
    for AnvilState<BackendData>
{
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrScreencopyManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl<BackendData: Backend + 'static> Dispatch<ZwlrScreencopyManagerV1, ()> for AnvilState<BackendData> {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _manager: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput {
                frame,
                overlay_cursor,
                output,
            } => {
                let Some(output) = Output::from_resource(&output) else {
                    let frame = data_init.init(
                        frame,
                        FrameData {
                            output: dummy_output(),
                            buffer_size: Size::from((0, 0)),
                            region_loc: Point::from((0, 0)),
                            overlay_cursor: false,
                            copied: Arc::new(AtomicBool::new(true)),
                        },
                    );
                    frame.failed();
                    return;
                };
                let Some(buffer_size) = output_buffer_size(&output) else {
                    let frame = data_init.init(
                        frame,
                        FrameData {
                            output: output.clone(),
                            buffer_size: Size::from((0, 0)),
                            region_loc: Point::from((0, 0)),
                            overlay_cursor: false,
                            copied: Arc::new(AtomicBool::new(true)),
                        },
                    );
                    frame.failed();
                    return;
                };
                let frame = data_init.init(
                    frame,
                    FrameData {
                        output,
                        buffer_size,
                        region_loc: Point::from((0, 0)),
                        overlay_cursor: overlay_cursor != 0,
                        copied: Arc::new(AtomicBool::new(false)),
                    },
                );
                announce_buffers(&frame, buffer_size);
            }
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion {
                frame,
                overlay_cursor,
                x,
                y,
                width,
                height,
                output,
            } => {
                if width <= 0 || height <= 0 {
                    let frame = data_init.init(
                        frame,
                        FrameData {
                            output: dummy_output(),
                            buffer_size: Size::from((0, 0)),
                            region_loc: Point::from((0, 0)),
                            overlay_cursor: false,
                            copied: Arc::new(AtomicBool::new(true)),
                        },
                    );
                    frame.failed();
                    return;
                }
                let Some(output) = Output::from_resource(&output) else {
                    let frame = data_init.init(
                        frame,
                        FrameData {
                            output: dummy_output(),
                            buffer_size: Size::from((0, 0)),
                            region_loc: Point::from((0, 0)),
                            overlay_cursor: false,
                            copied: Arc::new(AtomicBool::new(true)),
                        },
                    );
                    frame.failed();
                    return;
                };
                let Some(mode_size) = output_buffer_size(&output) else {
                    let frame = data_init.init(
                        frame,
                        FrameData {
                            output,
                            buffer_size: Size::from((0, 0)),
                            region_loc: Point::from((0, 0)),
                            overlay_cursor: false,
                            copied: Arc::new(AtomicBool::new(true)),
                        },
                    );
                    frame.failed();
                    return;
                };
                let transform = output.current_transform();
                let physical = transform.transform_size(mode_size);
                let output_rect = Rectangle::from_size(physical);
                let scale = output.current_scale().fractional_scale();
                let rect = Rectangle::new(Point::from((x, y)), Size::from((width, height)))
                    .to_physical_precise_round(scale);
                let Some(clamped) = rect.intersection(output_rect) else {
                    let frame = data_init.init(
                        frame,
                        FrameData {
                            output,
                            buffer_size: Size::from((0, 0)),
                            region_loc: Point::from((0, 0)),
                            overlay_cursor: false,
                            copied: Arc::new(AtomicBool::new(true)),
                        },
                    );
                    frame.failed();
                    return;
                };
                let untransformed = transform.invert().transform_rect_in(clamped, &physical);
                let buffer_size = untransformed.size;
                let frame = data_init.init(
                    frame,
                    FrameData {
                        output,
                        buffer_size,
                        region_loc: clamped.loc,
                        overlay_cursor: overlay_cursor != 0,
                        copied: Arc::new(AtomicBool::new(false)),
                    },
                );
                announce_buffers(&frame, buffer_size);
            }
            zwlr_screencopy_manager_v1::Request::Destroy => {}
            _ => unreachable!(),
        }
    }
}

impl<BackendData: Backend + 'static> Dispatch<ZwlrScreencopyFrameV1, FrameData>
    for AnvilState<BackendData>
{
    fn request(
        state: &mut Self,
        _client: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &FrameData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        if matches!(request, zwlr_screencopy_frame_v1::Request::Destroy) {
            return;
        }
        if data.copied.swap(true, Ordering::SeqCst) {
            frame.post_error(
                zwlr_screencopy_frame_v1::Error::AlreadyUsed,
                "copy already requested",
            );
            return;
        }
        let (buffer, with_damage) = match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => (buffer, false),
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => (buffer, true),
            _ => unreachable!(),
        };

        let stride = stride_for(data.buffer_size);
        let valid = shm::with_buffer_contents(&buffer, |_, _, meta| {
            meta.format == ShmFormat::Xrgb8888
                && meta.width == data.buffer_size.w
                && meta.height == data.buffer_size.h
                && meta.stride == stride
        })
        .unwrap_or(false);
        if !valid {
            frame.post_error(
                zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                "expected wl_shm Xrgb8888 matching announced size",
            );
            return;
        }

        trace!(
            output = %data.output.name(),
            with_damage,
            "screencopy copy queued"
        );
        state.screencopy_state.push(PendingScreencopy {
            frame: frame.clone(),
            output: data.output.clone(),
            buffer_size: data.buffer_size,
            region_loc: data.region_loc,
            overlay_cursor: data.overlay_cursor,
            with_damage,
            buffer: Some(buffer),
            submitted: false,
        });
    }
}
