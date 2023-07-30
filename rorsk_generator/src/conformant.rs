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
        let u32_id = self.get_op_type_int(false);
        let i32_3 = self.get_const_i32(3);

        // Replace op code id.
        let final_id = self.vec[self.i + 2];
        self.bound += 1;
        let op_id = self.bound;
        self.vec[self.i + 2] = op_id;

        // Bitcast to uint.
        self.bound += 1;
        let bitcast_id = self.bound;
        self.vec.insert(self.i + 5, 124 | (4 << 16));
        self.vec.insert(self.i + 6, u32_id);
        self.vec.insert(self.i + 7, bitcast_id);
        self.vec.insert(self.i + 8, op_id);

        // Logical shift to right.
        self.bound += 1;
        let right_shift_id = self.bound;
        self.vec.insert(self.i + 9, 194 | (5 << 16));
        self.vec.insert(self.i + 10, u32_id);
        self.vec.insert(self.i + 11, right_shift_id);
        self.vec.insert(self.i + 12, bitcast_id);
        self.vec.insert(self.i + 13, i32_3);

        // Logical shift to left.
        self.bound += 1;
        let left_shift_id = self.bound;
        self.vec.insert(self.i + 14, 196 | (5 << 16));
        self.vec.insert(self.i + 15, u32_id);
        self.vec.insert(self.i + 16, left_shift_id);
        self.vec.insert(self.i + 17, right_shift_id);
        self.vec.insert(self.i + 18, i32_3);

        // Bitcast to float.
        let final_type_id = self.vec[self.i + 1];
        self.vec.insert(self.i + 19, 124 | (4 << 16));
        self.vec.insert(self.i + 20, final_type_id);
        self.vec.insert(self.i + 21, final_id);
        self.vec.insert(self.i + 22, left_shift_id);
    }
}
