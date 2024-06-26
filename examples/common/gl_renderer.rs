//
// Copyright 2022-Present (c) Raja Lehtihet & Wael El Oraiby
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
// this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
// this list of conditions and the following disclaimer in the documentation
// and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
// may be used to endorse or promote products derived from this software without
// specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
//
// -----------------------------------------------------------------------------
// Ported to rust from https://github.com/rxi/microui/ and the original license
//
// Copyright (c) 2020 rxi
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
//

use microui_redux::*;
use glow::*;

const VERTEX_SHADER: &str = "#version 100
uniform highp mat4 uTransform;
attribute highp vec2 vertexPosition;
attribute highp vec2 vertexTexCoord;
attribute lowp vec4 vertexColor;
varying highp vec2 vTexCoord;
varying lowp vec4 vVertexColor;
void main()
{
    vVertexColor = vertexColor;
    vTexCoord = vertexTexCoord;
    highp vec4 pos = vec4(vertexPosition.x, vertexPosition.y, 0.0, 1.0);
    gl_Position = uTransform * pos;
}";

const FRAGMENT_SHADER: &str = "#version 100
varying highp vec2 vTexCoord;
varying lowp vec4 vVertexColor;
uniform sampler2D uTexture;
void main()
{
    lowp vec4 col = texture2D(uTexture, vTexCoord);
    gl_FragColor = col * vVertexColor;
}";

pub struct GLRenderer {
    gl: glow::Context,
    verts: Vec<Vertex>,
    indices: Vec<u16>,

    vbo: NativeBuffer,
    ibo: NativeBuffer,
    tex_o: NativeTexture,

    program: NativeProgram,

    width: u32,
    height: u32,

    atlas: AtlasHandle,
    last_update_id: usize,
}

impl GLRenderer {
    fn create_program(gl: &mut glow::Context, vertex_shader_source: &str, fragment_shader_source: &str) -> NativeProgram {
        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [(glow::VERTEX_SHADER, vertex_shader_source), (glow::FRAGMENT_SHADER, fragment_shader_source)];

            let mut shaders = Vec::with_capacity(shader_sources.len());

            for (shader_type, shader_source) in shader_sources.iter() {
                let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
                gl.shader_source(shader, shader_source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    panic!("{}", gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            program
        }
    }

    fn update_atlas(&mut self) {
        if self.last_update_id != self.atlas.get_last_update_id() {
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex_o));
                debug_assert!(self.gl.get_error() == 0);
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    self.atlas.width() as i32,
                    self.atlas.height() as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    Some(&self.atlas.pixels().iter().map(|c| [c.x, c.y, c.z, c.w]).flatten().collect::<Vec<u8>>()),
                );
                debug_assert!(self.gl.get_error() == 0);
            }
            self.last_update_id = self.atlas.get_last_update_id()
        }
    }

    pub fn new(mut gl: glow::Context, atlas: AtlasHandle, width: u32, height: u32) -> Self {
        assert_eq!(core::mem::size_of::<Vertex>(), 20);
        unsafe {
            // init texture
            let tex_o = gl.create_texture().unwrap();
            debug_assert!(gl.get_error() == 0);
            gl.bind_texture(glow::TEXTURE_2D, Some(tex_o));
            debug_assert!(gl.get_error() == 0);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                atlas.width() as i32,
                atlas.height() as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(&atlas.pixels().iter().map(|c| [c.x, c.y, c.z, c.w]).flatten().collect::<Vec<u8>>()),
            );
            debug_assert!(gl.get_error() == 0);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
            debug_assert!(gl.get_error() == 0);

            let vbo = gl.create_buffer().unwrap();
            let ibo = gl.create_buffer().unwrap();

            let program = Self::create_program(&mut gl, VERTEX_SHADER, FRAGMENT_SHADER);

            Self {
                gl,
                verts: Vec::new(),
                indices: Vec::new(),

                vbo,
                ibo,
                tex_o,
                program,

                width,
                height,
                atlas,
                last_update_id: usize::MAX,
            }
        }
    }
}

impl Renderer<()> for GLRenderer {
    fn command(&mut self, _pr: &()) {}
    fn get_atlas(&self) -> AtlasHandle {
        self.atlas.clone()
    }

    fn flush(&mut self) {
        self.update_atlas();
        if self.verts.len() == 0 || self.indices.len() == 0 {
            return;
        }

        unsafe {
            // opengl rendering states
            self.gl.viewport(0, 0, self.width as i32, self.height as i32);

            self.gl.enable(glow::BLEND);
            debug_assert!(self.gl.get_error() == 0);
            self.gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            debug_assert!(self.gl.get_error() == 0);
            self.gl.disable(glow::CULL_FACE);
            debug_assert!(self.gl.get_error() == 0);
            self.gl.disable(glow::DEPTH_TEST);
            debug_assert!(self.gl.get_error() == 0);
            self.gl.enable(glow::SCISSOR_TEST);
            debug_assert!(self.gl.get_error() == 0);

            // set the program
            self.gl.use_program(Some(self.program));
            debug_assert!(self.gl.get_error() == 0);

            // set the texture
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.tex_o));
            self.gl.active_texture(glow::TEXTURE0 + 0);
            let tex_uniform_id = self.gl.get_uniform_location(self.program, "uTexture").unwrap();
            self.gl.uniform_1_i32(Some(&tex_uniform_id), 0);
            debug_assert_eq!(self.gl.get_error(), 0);

            // set the viewport
            let viewport = self.gl.get_uniform_location(self.program, "uTransform").unwrap();
            let tm = ortho4(0.0, self.width as f32, self.height as f32, 0.0, -1.0, 1.0).col.as_ptr() as *const _ as *const f32;
            let slice = std::slice::from_raw_parts(tm, 16);
            self.gl.uniform_matrix_4_f32_slice(Some(&viewport), false, &slice);
            debug_assert_eq!(self.gl.get_error(), 0);

            // set the vertex buffer
            let pos_attrib_id = self.gl.get_attrib_location(self.program, "vertexPosition").unwrap();
            let tex_attrib_id = self.gl.get_attrib_location(self.program, "vertexTexCoord").unwrap();
            let col_attrib_id = self.gl.get_attrib_location(self.program, "vertexColor").unwrap();
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            self.gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ibo));
            debug_assert!(self.gl.get_error() == 0);

            // update the vertex buffer
            let vertices_u8: &[u8] = core::slice::from_raw_parts(self.verts.as_ptr() as *const u8, self.verts.len() * core::mem::size_of::<Vertex>());
            self.gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, vertices_u8, glow::DYNAMIC_DRAW);
            debug_assert!(self.gl.get_error() == 0);

            // update the index buffer
            let indices_u8: &[u8] = core::slice::from_raw_parts(self.indices.as_ptr() as *const u8, self.indices.len() * core::mem::size_of::<u16>());
            self.gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, indices_u8, glow::DYNAMIC_DRAW);
            debug_assert!(self.gl.get_error() == 0);

            self.gl.enable_vertex_attrib_array(pos_attrib_id);
            self.gl.enable_vertex_attrib_array(tex_attrib_id);
            self.gl.enable_vertex_attrib_array(col_attrib_id);
            debug_assert!(self.gl.get_error() == 0);

            self.gl.vertex_attrib_pointer_f32(pos_attrib_id, 2, glow::FLOAT, false, 20, 0);
            self.gl.vertex_attrib_pointer_f32(tex_attrib_id, 2, glow::FLOAT, false, 20, 8);
            self.gl.vertex_attrib_pointer_f32(col_attrib_id, 4, glow::UNSIGNED_BYTE, true, 20, 16);
            debug_assert!(self.gl.get_error() == 0);

            self.gl.draw_elements(glow::TRIANGLES, self.indices.len() as i32, glow::UNSIGNED_SHORT, 0);
            debug_assert!(self.gl.get_error() == 0);

            self.gl.disable_vertex_attrib_array(pos_attrib_id);
            self.gl.disable_vertex_attrib_array(tex_attrib_id);
            self.gl.disable_vertex_attrib_array(col_attrib_id);
            debug_assert!(self.gl.get_error() == 0);
            self.gl.use_program(None);
            debug_assert!(self.gl.get_error() == 0);

            self.verts.clear();
            self.indices.clear();
        }
    }

    fn push_quad_vertices(&mut self, v0: &Vertex, v1: &Vertex, v2: &Vertex, v3: &Vertex) {
        if self.verts.len() + 4 >= 65536 || self.indices.len() + 6 >= 65536 {
            self.flush();
        }

        let is = self.verts.len() as u16;
        self.indices.push(is + 0);
        self.indices.push(is + 1);
        self.indices.push(is + 2);
        self.indices.push(is + 2);
        self.indices.push(is + 3);
        self.indices.push(is + 0);

        self.verts.push(v0.clone());
        self.verts.push(v1.clone());
        self.verts.push(v2.clone());
        self.verts.push(v3.clone());

        // This is needed so that the update happens immediately (not the most optimized way)
        if self.last_update_id != self.atlas.get_last_update_id() {
            self.flush()
        }
    }

    fn clear(&mut self, width: i32, height: i32, clr: Color) {
        unsafe {
            self.width = width as u32;
            self.height = height as u32;
            self.flush();
            self.gl
                .clear_color(clr.r as f32 / 255.0, clr.g as f32 / 255.0, clr.b as f32 / 255.0, clr.a as f32 / 255.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            debug_assert!(self.gl.get_error() == 0);
        }
    }
}
