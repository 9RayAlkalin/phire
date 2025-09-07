use super::{Anim, Resource};
use crate::ext::{source_of_image, ScaleType};
use anyhow::{Ok, Result};
use macroquad::prelude::*;
use miniquad::{Texture, TextureFormat, TextureParams, TextureWrap};
use video_rs::{Decoder, DecoderBuilder};
use std::{cell::RefCell, io::Write};
use tempfile::NamedTempFile;

thread_local! {
    static VIDEO_BUFFER: RefCell<Vec<u8>> = RefCell::default();
}

pub struct Video {
    decoder: Decoder,
    pub video_file: NamedTempFile,

    material: Material,
    tex_rgb: Texture2D,

    start_time: f32,
    scale_type: ScaleType,
    alpha: Anim<f32>,
    dim: Anim<f32>,
    frame_delta: f32,
    pub next_frame: usize,
    pub ended: bool,
}

fn new_tex_rgb(w: u32, h: u32) -> Texture2D {
    Texture2D::from_miniquad_texture(Texture::new_render_texture(
        unsafe { get_internal_gl() }.quad_context,
        TextureParams {
            width: w,
            height: h,
            format: TextureFormat::RGB8,
            filter: FilterMode::Linear,
            wrap: TextureWrap::Clamp,
        },
    ))
}

impl Video {
    pub fn new(data: Vec<u8>, start_time: f32, scale_type: ScaleType, alpha: Anim<f32>, dim: Anim<f32>) -> Result<Self> {
        video_rs::init().unwrap();
        let mut video_file = NamedTempFile::new()?;
        video_file.write_all(&data)?;
        drop(data);
        let decoder = DecoderBuilder::new(video_file.path()).build()?;
        let frame_delta = 1. / decoder.frame_rate();
        let size = decoder.size();
        let w = size.0;
        let h = size.1;

        let material = load_material(
            shader::VERTEX,
            shader::FRAGMENT,
            MaterialParams {
                pipeline_params: PipelineParams::default(),
                uniforms: Vec::new(),
                textures: vec!["tex_rgb".to_owned()],
            },
        )?;
        let tex_rgb = new_tex_rgb(w, h);
        material.set_texture("tex_rgb", tex_rgb);

        Ok(Self {
            decoder,
            video_file,

            material,
            tex_rgb,

            start_time,
            scale_type,
            alpha,
            dim,
            frame_delta,
            next_frame: 0,
            ended: false,
        })
    }

    pub fn update(&mut self, t: f32) -> Result<()> {
        if t < self.start_time || self.ended {
            return Ok(());
        }
        self.alpha.set_time(t);
        self.dim.set_time(t);
        let that_frame = ((t - self.start_time) / self.frame_delta) as usize;
        if self.next_frame <= that_frame {
            VIDEO_BUFFER.with(|it| {
                let mut buf = it.borrow_mut();
                while self.next_frame <= that_frame {
                    if let ::std::result::Result::Ok((_, frame)) = self.decoder.decode() {
                        if self.next_frame < that_frame {
                            self.next_frame += 1;
                            continue;
                        }
                        let rgb = frame.as_slice().unwrap();
                        buf.clear();
                        buf.extend_from_slice(rgb);
                    } else {
                        self.ended = true;
                        return;
                    }
                    self.next_frame += 1;
                }

                let ctx = unsafe { get_internal_gl() }.quad_context;
                self.tex_rgb.raw_miniquad_texture_handle().update(ctx, &buf);
            });
        }
        Ok(())
    }

    pub fn render(&self, res: &Resource) {
        if res.time < self.start_time || self.ended {
            return;
        }
        gl_use_material(self.material);
        let top = 1. / res.aspect_ratio;
        let r = Rect::new(-1., -top, 2., top * 2.);
        let s = source_of_image(&self.tex_rgb, r, self.scale_type).unwrap_or_else(|| Rect::new(0., 0., 1., 1.));
        let dim = 1. - self.dim.now();
        let color = Color::new(dim, dim, dim, self.alpha.now_opt().unwrap_or(1.));
        let vertices = [
            Vertex::new(r.x, r.y, 0., s.x, s.y, color),
            Vertex::new(r.right(), r.y, 0., s.right(), s.y, color),
            Vertex::new(r.x, r.bottom(), 0., s.x, s.bottom(), color),
            Vertex::new(r.right(), r.bottom(), 0., s.right(), s.bottom(), color),
        ];
        let gl = unsafe { get_internal_gl() }.quad_gl;
        gl.draw_mode(DrawMode::Triangles);
        gl.geometry(&vertices, &[0, 2, 3, 0, 1, 3]);
        gl_use_default_material();
    }

    pub fn reset(&mut self) -> Result<()> {
        self.next_frame = 0;
        self.ended = false;
        self.decoder.seek_to_start()?;
        Ok(())
    }
}

mod shader {
    pub const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

    pub const FRAGMENT: &str = r#"#version 100
precision lowp float;

varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D tex_rgb;

void main() {
    vec3 rgb = texture2D(tex_rgb, uv).rgb;
    gl_FragColor = vec4(rgb, 1.0) * color;
}"#;
}
