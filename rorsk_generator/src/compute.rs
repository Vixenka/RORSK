use core::slice;
use std::{time::Instant, fs::{self, File}, mem, io::{Write, Read}, path::Path};

use vulkano::buffer::BufferContents;

use crate::{runner, conformant};

pub(crate) struct Compute<T> where T: BufferContents + Clone {
    initial_data: Vec<T>,
}

impl<T> Compute<T> where T: BufferContents + Clone {
    pub(crate) fn new(initial_data: Vec<T>) -> Self {
        let vec = unsafe {
            slice::from_raw_parts::<u8>(initial_data.as_ptr() as *const u8, initial_data.len() * mem::size_of::<T>())
        };
        let sha256 = sha256::digest(vec);
        println!("Loaded initial data. SHA256: `{sha256}`.");

        Compute {
            initial_data,
        }
    }

    pub(crate) fn compute(&self, problem_name: &str, glsl_type: &str, expression: &str) {
        let offset = self.initial_data.len() / 2;

        let mut spirv_code = Vec::new();
        glsl_to_spirv::compile(&format!("{}{glsl_type}{}{glsl_type}{}{glsl_type}{}{offset}{}{glsl_type}{}{expression}{}", r#"
            #version 450

            layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

            layout(set = 0, binding = 0) buffer Data {
                "#, r#" data[];
            };

            void main() {
                "#, r#" a = data[gl_GlobalInvocationID.x];
                "#, r#" b = data[gl_GlobalInvocationID.x + "#, r#"];
                "#, r#" r;

                "#, r#"

                data[gl_GlobalInvocationID.x] = r;
            }
        "#), glsl_to_spirv::ShaderType::Compute).unwrap().read_to_end(&mut spirv_code).unwrap();

        /*glsl_to_spirv::compile(r#"
            #version 450
            #extension GL_ARB_gpu_shader_int64 : enable

            int FromFractionAndExp(int traw32, int exp) {
                if (exp < 0)
                    return 0;

                exp = min(exp, 255);
                return (traw32 << 8) | (exp & 0xFF);
            }

            int FromFloat(float value) {
                if (value == 0)
                    return 0;

                int t754raw = floatBitsToInt(value);
                int tRaction = (t754raw & 0x007FFFFF) + 0x00800000;
                int exponent = (t754raw & 0x7FFFFFFF) >> 23;

                if (t754raw < 0)
                    tRaction = -tRaction;

                return FromFractionAndExp(tRaction >> 1, exponent - 22);
            }

            int GetFraction(int gfloat) {
                return gfloat >> 8;
            }

            int GetExponent(int gfloat) {
                return gfloat & 0xFF;
            }

            float ToFloat(int gfloat) {
                int exponent = GetExponent(gfloat) - 127;
                float dt = pow(2, exponent);
                return float(GetFraction(gfloat)) * dt;
            }

            int GBitScanReverse64(int64_t num) {
                for (int i = 63; i >= 0; i--) {
                    if ((num & (int64_t(1) << int64_t(i))) != 0)
                        return i;
                }
                return 0;
            }

            int Normalize64(int64_t trawValue, int tExponent) {
                if (trawValue == 0)
                    return 0;

                int index = GBitScanReverse64(abs(trawValue));
                if (index <= 22) {
                    int uDelta = 22 - index;
                    return FromFractionAndExp(int(trawValue << uDelta), tExponent - uDelta);
                } else {
                    int uDelta = index - 22;
                    return FromFractionAndExp(int(trawValue >> uDelta), tExponent + uDelta);
                }
            }

            int GDiv(int lhs, int rhs) {
                int nDivid = GetFraction(rhs);
                if (nDivid == 0)
                    return 0;

                int64_t trawValue = (int64_t(GetFraction(lhs)) << 32) / nDivid;
                int tExponent = GetExponent(lhs) - GetExponent(rhs) + 95;

                return Normalize64(trawValue, tExponent);
            }

            float Div(float lhs, float rhs) {
                return ToFloat(GDiv(FromFloat(lhs), FromFloat(rhs)));
            }

            layout(local_size_x = 64, local_size_y = 1, local_size_z = 1) in;

            layout(set = 0, binding = 0) buffer Data {
                float data[];
            };

            void main() {
                float a = data[gl_GlobalInvocationID.x];
                float b = data[gl_GlobalInvocationID.x + 4000000];

                float r = Div(a, b);

                data[gl_GlobalInvocationID.x] = r;
            }
        "#, glsl_to_spirv::ShaderType::Compute).unwrap().read_to_end(&mut spirv_code).unwrap();*/

        self.compute_impl(problem_name, &spirv_code, false);

        let conformant = conformant::process(spirv_code);
        self.compute_impl(problem_name, &conformant, true);
    }

    fn compute_impl(&self, problem_name: &str, spirv_code: &[u8], is_conformant: bool) {
        let now = Instant::now();

        let conformant_str = if is_conformant { "conformant" } else { "unconformant" };
        println!("Computing {conformant_str} data from problem named `{problem_name}`...");

        let output = runner::run::<T>(spirv_code, &self.initial_data, self.initial_data.len() / 64 / 2);
        println!("Done in {} ms.", now.elapsed().as_millis());

        let conformant_str = if is_conformant { "binc" } else { "bin" };
        let path = format!("../output/{problem_name}_{0}_{1}.{conformant_str}", output.device_vendor_id, output.device_id);

        fs::create_dir_all("../output").unwrap();
        let mut file = File::create(path.clone()).unwrap();
        file.write_all(unsafe {
            slice::from_raw_parts::<u8>(output.data.as_ptr() as *const u8, output.data.len() * mem::size_of::<T>())
        }).unwrap();

        let sha256 = sha256::try_digest(Path::new(&path)).unwrap();
        println!("Saved result data to `{path}`. SHA256: `{sha256}`.");
    }
}
