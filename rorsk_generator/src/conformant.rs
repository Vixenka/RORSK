use std::mem;

pub fn process(spirv: Vec<u8>) -> Vec<u8> {
    let mut vec = unsafe {
        let ptr = spirv.as_ptr() as *mut u32;
        let length = spirv.len() / 4;
        let capacity = spirv.capacity() / 4;

        mem::forget(spirv);
        Vec::from_raw_parts(ptr, length, capacity)
    };

    Buffer::new(&mut vec).process();

    unsafe {
        let ptr = vec.as_mut_ptr() as *mut u8;
        let length = vec.len() * 4;
        let capacity = vec.capacity() * 4;

        mem::forget(vec);
        Vec::from_raw_parts(ptr, length, capacity)
    }
}

struct Buffer<'a> {
    vec: &'a mut Vec<u32>,
    op_type_index: usize,
    bound: u32,
    i: usize,
}

impl<'a> Buffer<'a> {
    fn new(vec: &'a mut Vec<u32>) -> Self {
        Buffer {
            vec,
            op_type_index: usize::MAX,
            bound: 0,
            i: 0,
        }
    }

    fn process(&mut self) {
        self.bound = self.vec[3] - 1;

        self.i = 5;
        while self.i < self.vec.len() {
            let word_count = self.vec[self.i] >> 16;

            match self.vec[self.i] & 0xFFFF {
                // OpTypeVoid
                19 => self.op_type_index = self.i,
                // OpFDiv
                136 => self.op_fdiv(),
                _ => {}
            };

            self.i += word_count as usize;
        }

        self.vec[3] = self.bound + 1;
    }

    fn get_op_type_int(&mut self, signedness: bool) -> u32 {
        self.bound += 1;
        let id = self.bound;
        self.vec.insert(self.op_type_index, 21 | (4 << 16));
        self.vec.insert(self.op_type_index + 1, id);
        self.vec.insert(self.op_type_index + 2, 32);
        self.vec.insert(self.op_type_index + 3, match signedness {
            true => 1,
            false => 0,
        });

        self.i += 4;
        self.op_type_index += 4;

        id
    }

    fn get_op_type_float(&mut self, width: u32) -> u32 {
        if width == 64 {
            // Enable Float64 capabilty.
            self.vec.insert(7, 17 | (2 << 16));
            self.vec.insert(8, 10);

            self.i += 2;
            self.op_type_index += 2;
        }

        self.bound += 1;
        let id = self.bound;
        self.vec.insert(self.op_type_index, 22 | (3 << 16));
        self.vec.insert(self.op_type_index + 1, id);
        self.vec.insert(self.op_type_index + 2, width);

        self.i += 3;
        self.op_type_index += 3;

        id
    }

    fn get_const_i32(&mut self, value: u32) -> u32 {
        let i32_id = self.get_op_type_int(true);

        self.bound += 1;
        let id = self.bound;
        self.vec.insert(self.op_type_index, 43 | (4 << 16));
        self.vec.insert(self.op_type_index + 1, i32_id);
        self.vec.insert(self.op_type_index + 2, id);
        self.vec.insert(self.op_type_index + 3, value);

        self.i += 4;
        self.op_type_index += 4;

        id
    }

    fn op_fdiv(&mut self) {
        let u64_id = self.get_op_type_float(64);

        let original_lhs_id = self.vec[self.i + 3];
        let original_rhs_id = self.vec[self.i + 4];

        // Convert parameters to f64.
        self.bound += 1;
        let lhs_id = self.bound;
        self.vec.insert(self.i, 115 | (4 << 16));
        self.vec.insert(self.i + 1, u64_id);
        self.vec.insert(self.i + 2, lhs_id);
        self.vec.insert(self.i + 3, original_lhs_id);

        self.bound += 1;
        let rhs_id = self.bound;
        self.vec.insert(self.i + 4, 115 | (4 << 16));
        self.vec.insert(self.i + 5, u64_id);
        self.vec.insert(self.i + 6, rhs_id);
        self.vec.insert(self.i + 7, original_rhs_id);

        self.i += 8;

        // Replace op code id.
        let final_id = self.vec[self.i + 2];
        self.bound += 1;
        let op_id = self.bound;
        self.vec[self.i + 2] = op_id;

        // Replace op code type.
        let final_type_id = self.vec[self.i + 1];
        self.vec[self.i + 1] = u64_id;

        // Replace parameters.
        self.vec[self.i + 3] = lhs_id;
        self.vec[self.i + 4] = rhs_id;

        // Convert result to f32.
        self.vec.insert(self.i + 5, 115 | (4 << 16));
        self.vec.insert(self.i + 6, final_type_id);
        self.vec.insert(self.i + 7, final_id);
        self.vec.insert(self.i + 8, op_id);
    }
}
