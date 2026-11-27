//! GL curve for the Audiophile tab. The CPU `DrawingArea` stays when GL
//! cannot be created or the shader does not link.

use gtk4::prelude::*;
use gtk4::GLArea;

extern "C" {
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const i8) -> *mut std::ffi::c_void;
}

pub fn painter_label(gl_ok: bool) -> &'static str {
    if gl_ok {
        "Curve painter: GL"
    } else {
        "Curve painter: CPU"
    }
}

fn load_symbol(name: &str) -> *const std::ffi::c_void {
    let Ok(symbol) = std::ffi::CString::new(name) else {
        return std::ptr::null();
    };
    unsafe { dlsym(std::ptr::null_mut(), symbol.as_ptr()) as *const std::ffi::c_void }
}

fn ensure_gl_loaded() -> bool {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        gl::load_with(load_symbol);
    });
    !load_symbol("glClear").is_null() && !load_symbol("glCreateShader").is_null()
}

fn curve_points() -> Vec<f32> {
    let mut eq = crate::eq::shared_eq().lock();
    let n = 96;
    let mut points = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f64 / (n - 1) as f64;
        let hz = 20.0 * 1000.0_f64.powf(t);
        let db = eq.response_db(hz).clamp(-24.0, 12.0);
        let x = t as f32 * 2.0 - 1.0;
        let y = ((12.0 - db) / 36.0 * 2.0 - 1.0) as f32;
        points.push(x);
        points.push(-y);
    }
    points
}

fn program() -> Option<u32> {
    static PROGRAM: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *PROGRAM.get_or_init(|| unsafe { link_program() })
}

unsafe fn link_program() -> Option<u32> {
    let vs = gl::CreateShader(gl::VERTEX_SHADER);
    let fs = gl::CreateShader(gl::FRAGMENT_SHADER);
    let vsrc = b"#version 150\nin vec2 pos;\nvoid main(){ gl_Position = vec4(pos, 0.0, 1.0); }\0";
    let fsrc = b"#version 150\nout vec4 color;\nvoid main(){ color = vec4(0.55, 0.82, 1.0, 1.0); }\0";
    shader_source(vs, vsrc);
    shader_source(fs, fsrc);
    gl::CompileShader(vs);
    gl::CompileShader(fs);
    if !shader_ok(vs) || !shader_ok(fs) {
        return None;
    }
    let program = gl::CreateProgram();
    gl::AttachShader(program, vs);
    gl::AttachShader(program, fs);
    gl::BindAttribLocation(program, 0, b"pos\0".as_ptr() as *const i8);
    gl::LinkProgram(program);
    let mut status = 0;
    gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);
    if status == 0 {
        None
    } else {
        Some(program)
    }
}

unsafe fn shader_source(shader: u32, src: &[u8]) {
    let ptr = src.as_ptr() as *const i8;
    gl::ShaderSource(shader, 1, &ptr, std::ptr::null());
}

unsafe fn shader_ok(shader: u32) -> bool {
    let mut status = 0;
    gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);
    status != 0
}

/// Returns whether this frame was drawn with GL.
pub fn render_curve(area: &GLArea) -> bool {
    area.attach_buffers();
    if area.error().is_some() || !ensure_gl_loaded() {
        return false;
    }
    let Some(program) = program() else {
        return false;
    };
    let points = curve_points();
    unsafe {
        gl::Viewport(0, 0, area.width().max(1), area.height().max(1));
        gl::ClearColor(0.05, 0.06, 0.08, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
        gl::UseProgram(program);
        let mut buffer = 0;
        gl::GenBuffers(1, &mut buffer);
        gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (points.len() * std::mem::size_of::<f32>()) as isize,
            points.as_ptr() as *const _,
            gl::STREAM_DRAW,
        );
        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 0, std::ptr::null());
        gl::DrawArrays(gl::LINE_STRIP, 0, (points.len() / 2) as i32);
        gl::DeleteBuffers(1, &buffer);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::painter_label;

    #[test]
    fn painter_name_records_the_fallback() {
        assert_eq!(painter_label(true), "Curve painter: GL");
        assert_eq!(painter_label(false), "Curve painter: CPU");
    }
}
