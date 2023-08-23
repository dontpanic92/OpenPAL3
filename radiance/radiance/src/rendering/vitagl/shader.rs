use std::ffi::CString;

use crate::rendering::{shader::ShaderProgramData, Shader, ShaderProgram, VertexComponents};

pub struct VitaGLShader {
    name: String,
    vertex_shader: u32,
    fragment_shader: u32,
    program: u32,
    uniform_model_matrix: i32,
    uniform_view_matrix: i32,
    uniform_projection_matrix: i32,
}

impl Shader for VitaGLShader {
    fn name(&self) -> &str {
        &self.name
    }
}

impl VitaGLShader {
    pub fn new(shader: ShaderProgram) -> anyhow::Result<Self> {
        let data = get_shader_proram_data(shader);
        log::info!("loading shader {}", data.name);
        unsafe {
            use vitagl_sys::*;
            let vertex_shader = glCreateShader(GL_VERTEX_SHADER);
            let fragment_shader = glCreateShader(GL_FRAGMENT_SHADER);
            let program = glCreateProgram();
            let vert_src = CString::new(data.vert_src).unwrap();
            let frag_src = CString::new(data.frag_src).unwrap();
            let vsize = data.vert_src.len() as i32;
            let fsize = data.frag_src.len() as i32;

            glShaderSource(vertex_shader, 1, &vert_src.as_ptr(), &vsize);
            glCompileShader(vertex_shader);
            print_shader_log(vertex_shader);

            glShaderSource(fragment_shader, 1, &frag_src.as_ptr(), &fsize);
            glCompileShader(fragment_shader);
            print_shader_log(fragment_shader);

            glAttachShader(program, vertex_shader);
            glAttachShader(program, fragment_shader);

            let position = CString::new("position").unwrap();
            let texcoord = CString::new("texcoord").unwrap();
            glBindAttribLocation(program, 0, position.as_ptr());
            glBindAttribLocation(program, 1, texcoord.as_ptr());

            glLinkProgram(program);

            let sampler = CString::new("texSampler").unwrap();
            glUniform1i(glGetUniformLocation(program, sampler.as_ptr()), 0);

            if data.components.contains(VertexComponents::TEXCOORD2) {
                let texcoord2 = CString::new("texcoord2").unwrap();
                glBindAttribLocation(program, 2, texcoord2.as_ptr());

                let sampler2 = CString::new("texSampler2").unwrap();
                glUniform1i(glGetUniformLocation(program, sampler2.as_ptr()), 1);
            }

            let model_matrix = CString::new("modelMatrix").unwrap();
            let uniform_model_matrix = glGetUniformLocation(program, model_matrix.as_ptr());

            let view_matrix = CString::new("viewMatrix").unwrap();
            let uniform_view_matrix = glGetUniformLocation(program, view_matrix.as_ptr());

            let projection_matrix = CString::new("projectionMatrix").unwrap();
            let uniform_projection_matrix =
                glGetUniformLocation(program, projection_matrix.as_ptr());

            Ok(Self {
                name: data.name.to_owned(),
                vertex_shader,
                fragment_shader,
                program,
                uniform_model_matrix,
                uniform_projection_matrix,
                uniform_view_matrix,
            })
        }
    }

    pub fn program(&self) -> u32 {
        self.program
    }

    pub fn uniform_model_matrix(&self) -> i32 {
        self.uniform_model_matrix
    }

    pub fn uniform_view_matrix(&self) -> i32 {
        self.uniform_view_matrix
    }

    pub fn uniform_projection_matrix(&self) -> i32 {
        self.uniform_projection_matrix
    }
}

impl Drop for VitaGLShader {
    fn drop(&mut self) {
        unsafe {
            use vitagl_sys::*;
            glDeleteShader(self.vertex_shader);
            glDeleteShader(self.fragment_shader);
            glDeleteProgram(self.program);
        }
    }
}

fn get_shader_proram_data(shader: ShaderProgram) -> ShaderProgramData {
    match shader {
        ShaderProgram::TexturedNoLight => ShaderProgramData::new(
            "TexturedNoLight",
            include_bytes!("shaders/simple_triangle.vert"),
            include_bytes!("shaders/simple_triangle.frag"),
            VertexComponents::POSITION | VertexComponents::TEXCOORD,
        ),
        ShaderProgram::TexturedLightmap => ShaderProgramData::new(
            "TexturedLightmap",
            include_bytes!("shaders/lightmap_texture.vert"),
            include_bytes!("shaders/lightmap_texture.frag"),
            VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2,
        ),
    }
}

fn print_shader_log(shader: u32) {
    unsafe {
        use vitagl_sys::*;
        let mut log_length = 0;
        glGetShaderiv(shader, GL_INFO_LOG_LENGTH, &mut log_length);
        let mut log = vec![0u8; log_length as usize];
        glGetShaderInfoLog(
            shader,
            log_length,
            std::ptr::null_mut(),
            log.as_mut_ptr() as *mut _,
        );
        log::error!("shader log: {}", std::str::from_utf8(&log).unwrap());
    }
}
