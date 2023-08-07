use std::{mem, collections::HashMap};

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
    created: HashMap<String, u32>,
    op_type_index: usize,
    op_function: usize,
    last_op_label: usize,
    bound: u32,
    i: usize,
}

impl<'a> Buffer<'a> {
    fn new(vec: &'a mut Vec<u32>) -> Self {
        Buffer {
            vec,
            created: HashMap::new(),
            op_type_index: usize::MAX,
            op_function: usize::MAX,
            last_op_label: usize::MAX,
            bound: 0,
            i: 0,
        }
    }

    fn process(&mut self) {
        self.bound = self.vec[3] - 1;

        self.i = 5;
        while self.i < self.vec.len() {
            let word_count = self.vec[self.i] >> 16;
            let opcode = self.vec[self.i] & 0xFFFF;

            match opcode {
                // OpType*
                19..=39 => {
                    self.op_type_index = self.i;
                    match opcode {
                        // OpTypeInt
                        21 => {
                            let name = format!("int-{}-{}", self.vec[self.i + 2], self.vec[self.i + 3]);
                            self.created.insert(name, self.vec[self.i + 1]);
                        },
                        // OpTypeFloat
                        22 => {
                            let name = format!("float-{}", self.vec[self.i + 2]);
                            self.created.insert(name, self.vec[self.i + 1]);
                        },
                        _ => {}
                    }
                },
                // OpFunction
                54 => self.op_function = self.i,
                // OpFDiv
                136 => self.op_fdiv(),
                // OpLabel
                248 => self.last_op_label = self.i,
                _ => {}
            };

            self.i += word_count as usize;
        }

        self.vec[3] = self.bound + 1;
    }

    fn op_fdiv(&mut self) {
        let f32_conformant_div = self.f32_conformant_div();

        let f32 = self.get_op_type_float(32);
        let pf32 = self.get_pointer_type(f32, 7);

        // Create variables.
        let lhs = self.get_next_id();
        self.vec.insert(self.last_op_label + 2, 59 | (4 << 16));
        self.vec.insert(self.last_op_label + 3, pf32);
        self.vec.insert(self.last_op_label + 4, lhs);
        self.vec.insert(self.last_op_label + 5, 7);

        let rhs = self.get_next_id();
        self.vec.insert(self.last_op_label + 6, 59 | (4 << 16));
        self.vec.insert(self.last_op_label + 7, pf32);
        self.vec.insert(self.last_op_label + 8, rhs);
        self.vec.insert(self.last_op_label + 9, 7);

        self.move_pointer(self.op_function, 8);

        // Copy parameters to variables.
        let lhs_data = self.vec[self.i + 3];
        let rhs_data = self.vec[self.i + 4];

        self.vec.insert(self.i, 62 | (3 << 16));
        self.vec.insert(self.i + 1, lhs);
        self.vec.insert(self.i + 2, lhs_data);

        self.vec.insert(self.i + 3, 62 | (3 << 16));
        self.vec.insert(self.i + 4, rhs);
        self.vec.insert(self.i + 5, rhs_data);

        self.move_pointer(self.op_function, 6);

        // Execute conformant div.
        self.vec[self.i] = 57 | (6 << 16);
        self.vec.insert(self.i + 3, f32_conformant_div);
        self.vec[self.i + 4] = lhs;
        self.vec[self.i + 5] = rhs;

        self.move_pointer(self.i, 1);
    }

    fn move_pointer(&mut self, from: usize, value: usize) {
        if from <= self.op_type_index {
            self.op_type_index += value;
        }
        if from <= self.op_function {
            self.op_function += value;
        }
        if from <= self.last_op_label {
            self.last_op_label += value;
        }
        self.i += value;
    }

    fn get_next_id(&mut self) -> u32 {
        self.bound += 1;
        self.bound
    }

    fn get_op_type_bool(&mut self) -> u32 {
        let name = "bool".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let id = self.get_next_id();
        self.vec.insert(self.op_type_index, 20 | (2 << 16));
        self.vec.insert(self.op_type_index + 1, id);

        self.move_pointer(self.op_type_index, 2);

        self.created.insert(name, id);
        id
    }

    fn get_op_type_int(&mut self, width: u32, signedness: bool) -> u32 {
        let name = format!("int-{}-{}", width, if signedness { "1" } else { "0" });
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        if width == 64 {
            // Enable Int64 capabilty.
            self.vec.insert(7, 17 | (2 << 16));
            self.vec.insert(8, 11);

            self.move_pointer(7, 2);
        }

        let id = self.get_next_id();
        self.vec.insert(self.op_type_index, 21 | (4 << 16));
        self.vec.insert(self.op_type_index + 1, id);
        self.vec.insert(self.op_type_index + 2, width);
        self.vec.insert(self.op_type_index + 3, match signedness {
            true => 1,
            false => 0,
        });

        self.move_pointer(self.op_type_index, 4);

        self.created.insert(name, id);
        id
    }

    fn get_op_type_float(&mut self, width: u32) -> u32 {
        let name = format!("float-{}", width);
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let id = self.get_next_id();
        self.vec.insert(self.op_type_index, 22 | (3 << 16));
        self.vec.insert(self.op_type_index + 1, id);
        self.vec.insert(self.op_type_index + 2, width);

        self.move_pointer(self.op_type_index, 3);

        self.created.insert(name, id);
        id
    }

    fn get_const_int(&mut self, width: u32, signedness: bool, value: u32) -> u32 {
        let name = format!("int-{}-{}-{}", width, if signedness { "1" } else { "0" }, value);
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let i_id = self.get_op_type_int(width, signedness);

        let id = self.get_next_id();
        let length = if width == 64 { 5 } else { 4 };

        self.vec.insert(self.op_type_index, 43 | (length << 16));
        self.vec.insert(self.op_type_index + 1, i_id);
        self.vec.insert(self.op_type_index + 2, id);

        if width == 64 {
            self.vec.insert(self.op_type_index + 3, value);
            self.vec.insert(self.op_type_index + 4, 0);
        } else {
            self.vec.insert(self.op_type_index + 3, value);
        }

        self.move_pointer(self.op_type_index, length as usize);

        self.created.insert(name, id);
        id
    }

    fn get_const_f32(&mut self, width: u32, value: f32) -> u32 {
        let name = format!("float-{}-{}", width, value);
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let f32_id = self.get_op_type_float(width);

        let id = self.get_next_id();
        self.vec.insert(self.op_type_index, 43 | (4 << 16));
        self.vec.insert(self.op_type_index + 1, f32_id);
        self.vec.insert(self.op_type_index + 2, id);
        self.vec.insert(self.op_type_index + 3, value.to_bits());

        self.move_pointer(self.op_type_index, 4);

        self.created.insert(name, id);
        id
    }

    fn get_type_function(&mut self, return_type: u32, parameter_types: &[u32]) -> u32 {
        let name = format!("function-{}-{:?}", return_type, parameter_types);
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let id = self.get_next_id();

        self.vec.insert(self.op_type_index, 33 | ((3 + parameter_types.len()) << 16) as u32);
        self.vec.insert(self.op_type_index + 1, id);
        self.vec.insert(self.op_type_index + 2, return_type);
        for (i, &parameter_type) in parameter_types.iter().enumerate() {
            self.vec.insert(self.op_type_index + 3 + i, parameter_type);
        }

        self.move_pointer(self.op_type_index, 3 + parameter_types.len());

        self.created.insert(name, id);
        id
    }

    fn get_pointer_type(&mut self, type_: u32, storage_class: u32) -> u32 {
        let name = format!("pointer-{}-{}", type_, storage_class);
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let id = self.get_next_id();

        self.vec.insert(self.op_type_index, 32 | (4 << 16));
        self.vec.insert(self.op_type_index + 1, id);
        self.vec.insert(self.op_type_index + 2, storage_class);
        self.vec.insert(self.op_type_index + 3, type_);

        self.move_pointer(self.op_type_index, 4);

        self.created.insert(name, id);
        id
    }

    fn insert_op_function(&mut self, result_type_id: u32, function_control: u32, function_type_id: u32) -> u32 {
        let id = self.get_next_id();

        self.vec.insert(self.op_function, 54 | (5 << 16));
        self.vec.insert(self.op_function + 1, result_type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, function_control);
        self.vec.insert(self.op_function + 4, function_type_id);

        self.move_pointer(self.op_function, 5);

        id
    }

    fn insert_op_function_parameter(&mut self, type_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 55 | (3 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);

        self.move_pointer(self.op_function, 3);
        id
    }

    fn insert_op_label(&mut self, id: u32) {
        self.vec.insert(self.op_function, 248 | (2 << 16));
        self.vec.insert(self.op_function + 1, id);

        self.move_pointer(self.op_function, 2);
    }

    fn insert_op_branch(&mut self, target_label: u32) {
        self.vec.insert(self.op_function, 249 | (2 << 16));
        self.vec.insert(self.op_function + 1, target_label);

        self.move_pointer(self.op_function, 2);
    }

    fn insert_op_load(&mut self, type_id: u32, variable_pointer: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 61 | (4 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, variable_pointer);

        self.move_pointer(self.op_function, 4);
        id
    }

    fn insert_op_store(&mut self, variable_pointer: u32, object: u32) {
        self.vec.insert(self.op_function, 62 | (3 << 16));
        self.vec.insert(self.op_function + 1, variable_pointer);
        self.vec.insert(self.op_function + 2, object);

        self.move_pointer(self.op_function, 3);
    }

    fn insert_op_undef(&mut self, result_type_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 1 | (3 << 16));
        self.vec.insert(self.op_function + 1, result_type_id);
        self.vec.insert(self.op_function + 2, id);

        self.move_pointer(self.op_function, 3);
        id
    }

    fn insert_op_sless_than(&mut self, type_id: u32, lhs_id: u32, rhs_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 177 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs_id);
        self.vec.insert(self.op_function + 4, rhs_id);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_f_ord_equal(&mut self, type_id: u32, lhs_id: u32, rhs_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 180 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs_id);
        self.vec.insert(self.op_function + 4, rhs_id);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_s_less_than_equal(&mut self, type_id: u32, lhs_id: u32, rhs_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 179 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs_id);
        self.vec.insert(self.op_function + 4, rhs_id);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_s_greater_than_equal(&mut self, type_id: u32, lhs_id: u32, rhs_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 175 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs_id);
        self.vec.insert(self.op_function + 4, rhs_id);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_i_equal(&mut self, type_id: u32, lhs_id: u32, rhs_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 170 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs_id);
        self.vec.insert(self.op_function + 4, rhs_id);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_i_not_equal(&mut self, type_id: u32, lhs_id: u32, rhs_id: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 171 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs_id);
        self.vec.insert(self.op_function + 4, rhs_id);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_selection_merge(&mut self, merge_block: u32, selection_control: u32) {
        self.vec.insert(self.op_function, 247 | (3 << 16));
        self.vec.insert(self.op_function + 1, merge_block);
        self.vec.insert(self.op_function + 2, selection_control);

        self.move_pointer(self.op_function, 3);
    }

    fn insert_op_branch_conditional(&mut self, condition: u32, true_label: u32, false_label: u32) {
        self.vec.insert(self.op_function, 250 | (4 << 16));
        self.vec.insert(self.op_function + 1, condition);
        self.vec.insert(self.op_function + 2, true_label);
        self.vec.insert(self.op_function + 3, false_label);

        self.move_pointer(self.op_function, 4);
    }

    fn insert_op_return_value(&mut self, value_id: u32) {
        self.vec.insert(self.op_function, 254 | (2 << 16));
        self.vec.insert(self.op_function + 1, value_id);

        self.move_pointer(self.op_function, 2);
    }

    fn insert_op_ext_inst(&mut self, type_id: u32, set: u32, instruction: u32, operands: &[u32]) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 12 | ((5 + operands.len() as u32) << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, set);
        self.vec.insert(self.op_function + 4, instruction);
        for (i, operand) in operands.iter().enumerate() {
            self.vec.insert(self.op_function + 5 + i, *operand);
        }

        self.move_pointer(self.op_function, 5 + operands.len());
        id
    }

    fn insert_op_i_add(&mut self, type_id: u32, lhs: u32, rhs: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 128 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs);
        self.vec.insert(self.op_function + 4, rhs);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_s_div(&mut self, type_id: u32, lhs: u32, rhs: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 135 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs);
        self.vec.insert(self.op_function + 4, rhs);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_i_sub(&mut self, type_id: u32, lhs: u32, rhs: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 130 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs);
        self.vec.insert(self.op_function + 4, rhs);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_f_mul(&mut self, type_id: u32, lhs: u32, rhs: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 133 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs);
        self.vec.insert(self.op_function + 4, rhs);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_shift_right_arithmetic(&mut self, type_id: u32, base: u32, shift: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 195 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, base);
        self.vec.insert(self.op_function + 4, shift);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_shift_left_logical(&mut self, type_id: u32, base: u32, shift: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 196 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, base);
        self.vec.insert(self.op_function + 4, shift);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_bitwise_or(&mut self, type_id: u32, lhs: u32, rhs: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 197 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs);
        self.vec.insert(self.op_function + 4, rhs);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_bitwise_and(&mut self, type_id: u32, lhs: u32, rhs: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 199 | (5 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, lhs);
        self.vec.insert(self.op_function + 4, rhs);

        self.move_pointer(self.op_function, 5);
        id
    }

    fn insert_op_function_end(&mut self) {
        self.vec.insert(self.op_function, 56 | (1 << 16));
        self.move_pointer(self.op_function, 1);
    }

    fn insert_op_variable(&mut self, type_id: u32, storage_class: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 59 | (4 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, storage_class);

        self.move_pointer(self.op_function, 4);
        id
    }

    fn insert_op_s_convert(&mut self, result_type_id: u32, signed_value: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 114 | (4 << 16));
        self.vec.insert(self.op_function + 1, result_type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, signed_value);

        self.move_pointer(self.op_function, 4);
        id
    }

    fn insert_op_bitcast(&mut self, type_id: u32, operand: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 124 | (4 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, operand);

        self.move_pointer(self.op_function, 4);
        id
    }

    fn insert_op_s_negate(&mut self, type_id: u32, operand: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 126 | (4 << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, operand);

        self.move_pointer(self.op_function, 4);
        id
    }

    fn insert_op_convert_s_to_f(&mut self, result_type_id: u32, signed_value: u32) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 111 | (4 << 16));
        self.vec.insert(self.op_function + 1, result_type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, signed_value);

        self.move_pointer(self.op_function, 4);
        id
    }

    fn insert_op_function_call(&mut self, type_id: u32, function_id: u32, operands: &[u32]) -> u32 {
        let id = self.get_next_id();
        self.vec.insert(self.op_function, 57 | ((4 + operands.len() as u32) << 16));
        self.vec.insert(self.op_function + 1, type_id);
        self.vec.insert(self.op_function + 2, id);
        self.vec.insert(self.op_function + 3, function_id);
        for (i, operand) in operands.iter().enumerate() {
            self.vec.insert(self.op_function + 4 + i, *operand);
        }

        self.move_pointer(self.op_function, 4 + operands.len());
        id
    }

    fn insert_op_loop_merge(&mut self, merge_block: u32, continue_target: u32, loop_control: u32) {
        self.vec.insert(self.op_function, 246 | (4 << 16));
        self.vec.insert(self.op_function + 1, merge_block);
        self.vec.insert(self.op_function + 2, continue_target);
        self.vec.insert(self.op_function + 3, loop_control);

        self.move_pointer(self.op_function, 4);
    }

    fn sf32_from_fraction_and_exp(&mut self) -> u32 {
        let name = "sf32_from_fraction_And_exp".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let bool = self.get_op_type_bool();
        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pi32, pi32]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let traw32 = self.insert_op_function_parameter(pi32);
        let exp = self.insert_op_function_parameter(pi32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        // if (exp < 0)
        let loaded_exp = self.insert_op_load(i32, exp);
        let temp = self.get_const_int(32, true, 0);
        let cmp = self.insert_op_sless_than(bool, loaded_exp, temp);
        let false_label = self.get_next_id();
        self.insert_op_selection_merge(false_label, 0x0);
        let true_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // return 0;
        self.insert_op_label(true_label);
        self.insert_op_return_value(temp);

        // exp = min(exp, 255);
        self.insert_op_label(false_label);
        let loaded_exp = self.insert_op_load(i32, exp);
        let temp = self.get_const_int(32, true, 255);
        let min = self.insert_op_ext_inst(i32, 1, 39, &[loaded_exp, temp]);
        self.insert_op_store(exp, min);

        // return (traw32 << 8) | exp;
        let loaded_traw32 = self.insert_op_load(i32, traw32);
        let temp = self.get_const_int(32, true, 8);
        let shift = self.insert_op_shift_left_logical(i32, loaded_traw32, temp);
        let loaded_exp = self.insert_op_load(i32, exp);
        let or = self.insert_op_bitwise_or(i32, shift, loaded_exp);
        self.insert_op_return_value(or);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn sf32_from_float(&mut self) -> u32 {
        let name = "sf32_from_float".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let sf32_from_fraction_and_exp = self.sf32_from_fraction_and_exp();

        let bool = self.get_op_type_bool();
        let f32 = self.get_op_type_float(32);
        let pf32 = self.get_pointer_type(f32, 7);
        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pf32]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let value = self.insert_op_function_parameter(pf32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        let t754raw = self.insert_op_variable(pi32, 7);
        let t_raction = self.insert_op_variable(pi32, 7);
        let exponent = self.insert_op_variable(pi32, 7);

        // if (value == 0)
        let loaded_value = self.insert_op_load(f32, value);
        let temp = self.get_const_f32(32, 0.0);
        let cmp = self.insert_op_f_ord_equal(bool, loaded_value, temp);
        let false_label = self.get_next_id();
        self.insert_op_selection_merge(false_label, 0x0);
        let true_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // return 0;
        self.insert_op_label(true_label);
        let temp = self.get_const_int(32, true, 0);
        self.insert_op_return_value(temp);

        // int t754raw = floatBitsToInt(value);
        self.insert_op_label(false_label);
        let loaded_value = self.insert_op_load(f32, value);
        let bitcast = self.insert_op_bitcast(i32, loaded_value);
        self.insert_op_store(t754raw, bitcast);

        // int tRaction = (t754raw & 0x007FFFFF) + 0x00800000;
        let loaded_t754raw = self.insert_op_load(i32, t754raw);
        let temp = self.get_const_int(32, true, 0x007FFFFF);
        let and = self.insert_op_bitwise_and(i32, loaded_t754raw, temp);
        let temp = self.get_const_int(32, true, 0x00800000);
        let add = self.insert_op_i_add(i32, and, temp);
        self.insert_op_store(t_raction, add);

        // int exponent = (t754raw & 0x7FFFFFFF) >> 23;
        let loaded_t754raw = self.insert_op_load(i32, t754raw);
        let temp = self.get_const_int(32, true, 0x7FFFFFFF);
        let and = self.insert_op_bitwise_and(i32, loaded_t754raw, temp);
        let temp = self.get_const_int(32, true, 23);
        let shift = self.insert_op_shift_right_arithmetic(i32, and, temp);
        self.insert_op_store(exponent, shift);

        // if (t754raw < 0)
        let loaded_t754raw = self.insert_op_load(i32, t754raw);
        let temp = self.get_const_int(32, true, 0);
        let cmp = self.insert_op_sless_than(bool, loaded_t754raw, temp);
        let false_label = self.get_next_id();
        self.insert_op_selection_merge(false_label, 0x0);
        let true_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // tRaction = -tRaction;
        self.insert_op_label(true_label);
        let loaded_t_raction = self.insert_op_load(i32, t_raction);
        self.insert_op_s_negate(i32, loaded_t_raction);
        self.insert_op_branch(false_label);

        // return FromFractionAndExp(tRaction >> 1, exponent - 22);
        self.insert_op_label(false_label);
        let loaded_t_raction = self.insert_op_load(i32, t_raction);
        let temp = self.get_const_int(32, true, 1);
        let shift = self.insert_op_shift_right_arithmetic(i32, loaded_t_raction, temp);
        self.insert_op_store(t_raction, shift);

        let loaded_exponent = self.insert_op_load(i32, exponent);
        let temp = self.get_const_int(32, true, 22);
        let sub = self.insert_op_i_sub(i32, loaded_exponent, temp);
        self.insert_op_store(exponent, sub);

        let result = self.insert_op_function_call(i32, sf32_from_fraction_and_exp, &[t_raction, exponent]);
        self.insert_op_return_value(result);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn sf32_get_fraction(&mut self) -> u32 {
        let name = "sf32_get_fraction".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pi32]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let sf32 = self.insert_op_function_parameter(pi32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        // return gfloat >> 8;
        let loaded_sf32 = self.insert_op_load(i32, sf32);
        let temp = self.get_const_int(32, true, 8);
        let shift = self.insert_op_shift_right_arithmetic(i32, loaded_sf32, temp);
        self.insert_op_return_value(shift);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn sf32_get_exponent(&mut self) -> u32 {
        let name = "sf32_get_exponent".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pi32]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let sf32 = self.insert_op_function_parameter(pi32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        // return gfloat & 0xFF;
        let loaded_sf32 = self.insert_op_load(i32, sf32);
        let temp = self.get_const_int(32, true, 0xFF);
        let and = self.insert_op_bitwise_and(i32, loaded_sf32, temp);
        self.insert_op_return_value(and);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn sf32_to_float(&mut self) -> u32 {
        let name = "sf32_to_float".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let sf32_get_exponent = self.sf32_get_exponent();
        let sf32_get_fraction = self.sf32_get_fraction();

        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);
        let f32 = self.get_op_type_float(32);
        let pf32 = self.get_pointer_type(f32, 7);

        // OpFunction
        let op_type_function = self.get_type_function(f32, &[pi32]);
        let id = self.insert_op_function(f32, 0x8, op_type_function);
        let sf32 = self.insert_op_function_parameter(pi32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        let exponent = self.insert_op_variable(pi32, 7);
        let fraction = self.insert_op_variable(pi32, 7);
        let dt = self.insert_op_variable(pf32, 7);

        // int exponent = GetExponent(gfloat) - 127;
        let loaded_sf32 = self.insert_op_load(i32, sf32);
        self.insert_op_store(exponent, loaded_sf32);
        let call = self.insert_op_function_call(i32, sf32_get_exponent, &[exponent]);
        let temp = self.get_const_int(32, true, 127);
        let sub = self.insert_op_i_sub(i32, call, temp);
        self.insert_op_store(exponent, sub);

        // float dt = pow(2, exponent);
        let loaded_exponent = self.insert_op_load(i32, exponent);
        let convert = self.insert_op_convert_s_to_f(f32, loaded_exponent);
        let temp = self.get_const_f32(32, 2.0);
        let pow = self.insert_op_ext_inst(f32, 1, 26, &[temp, convert]);
        self.insert_op_store(dt, pow);

        // return float(GetFraction(gfloat)) * dt;
        let loaded_sf32 = self.insert_op_load(i32, sf32);
        self.insert_op_store(fraction, loaded_sf32);
        let call = self.insert_op_function_call(i32, sf32_get_fraction, &[fraction]);
        let convert = self.insert_op_convert_s_to_f(f32, call);
        let loaded_dt = self.insert_op_load(f32, dt);
        let mul = self.insert_op_f_mul(f32, convert, loaded_dt);
        self.insert_op_return_value(mul);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn find_s_msb_64(&mut self) -> u32 {
        let name = "find_s_msb_64".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let bool = self.get_op_type_bool();
        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);
        let i64 = self.get_op_type_int(64, true);
        let pi64 = self.get_pointer_type(i64, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pi64]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let num = self.insert_op_function_parameter(pi64);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        let i = self.insert_op_variable(pi32, 7);

        // for (int i = 63; i >= 0; i--) {
        let temp = self.get_const_int(32, true, 63);
        self.insert_op_store(i, temp);
        let loop_start = self.get_next_id();
        self.insert_op_branch(loop_start);
        self.insert_op_label(loop_start);

        let loop_end = self.get_next_id();
        let continue_target = self.get_next_id();
        self.insert_op_loop_merge(loop_end, continue_target, 0);

        let body = self.get_next_id();
        self.insert_op_branch(body);
        self.insert_op_label(body);

        let loaded_i = self.insert_op_load(i32, i);
        let temp = self.get_const_int(32, true, 0);
        let cmp = self.insert_op_s_greater_than_equal(bool, loaded_i, temp);
        let true_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, loop_end);

        // if ((num & (int64_t(1) << int64_t(i))) != 0)
        self.insert_op_label(true_label);
        let loaded_num = self.insert_op_load(i64, num);
        let loaded_i = self.insert_op_load(i32, i);
        let convert = self.insert_op_s_convert(i64, loaded_i);
        let temp = self.get_const_int(64, true, 1);
        let shift = self.insert_op_shift_left_logical(i64, temp, convert);
        let and = self.insert_op_bitwise_and(i64, loaded_num, shift);

        let temp = self.get_const_int(64, true, 0);
        let cmp = self.insert_op_i_not_equal(bool, and, temp);
        let true_label = self.get_next_id();
        let false_label = self.get_next_id();
        self.insert_op_selection_merge(false_label, 0);
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // return i;
        self.insert_op_label(true_label);
        let loaded_i = self.insert_op_load(i32, i);
        self.insert_op_return_value(loaded_i);

        // Loop continuation
        self.insert_op_label(false_label);
        self.insert_op_branch(continue_target);
        self.insert_op_label(continue_target);
        let loaded_i = self.insert_op_load(i32, i);
        let temp = self.get_const_int(32, true, 1);
        let sub = self.insert_op_i_sub(i32, loaded_i, temp);
        self.insert_op_store(i, sub);
        self.insert_op_branch(loop_start);

        // return 0;
        self.insert_op_label(loop_end);
        let temp = self.get_const_int(32, true, 0);
        self.insert_op_return_value(temp);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn sf32_normalize_64(&mut self) -> u32 {
        let name = "sf32_normalize_64".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let find_s_msb_64 = self.find_s_msb_64();
        let sf32_from_fraction_and_exp = self.sf32_from_fraction_and_exp();

        let bool = self.get_op_type_bool();
        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);
        let i64 = self.get_op_type_int(64, true);
        let pi64 = self.get_pointer_type(i64, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pi64, pi32]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let traw_value = self.insert_op_function_parameter(pi64);
        let t_exponent = self.insert_op_function_parameter(pi32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        let temp_i64 = self.insert_op_variable(pi64, 7);
        let index = self.insert_op_variable(pi32, 7);
        let param = self.insert_op_variable(pi32, 7);

        // if (trawValue == 0)
        let loaded_traw_value = self.insert_op_load(i64, traw_value);
        let temp = self.get_const_int(64, true, 0);
        let cmp = self.insert_op_i_equal(bool, loaded_traw_value, temp);
        let false_label = self.get_next_id();
        self.insert_op_selection_merge(false_label, 0);
        let true_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // return 0;
        self.insert_op_label(true_label);
        let temp = self.get_const_int(32, true, 0);
        self.insert_op_return_value(temp);

        // int index = GBitScanReverse64(abs(trawValue));
        self.insert_op_label(false_label);
        let loaded_traw_value = self.insert_op_load(i64, traw_value);
        let abs = self.insert_op_ext_inst(i64, 1, 5, &[loaded_traw_value]);
        self.insert_op_store(temp_i64, abs);
        let call = self.insert_op_function_call(i32, find_s_msb_64, &[temp_i64]);
        self.insert_op_store(index, call);

        // if (index <= 22) {
        let loaded_index = self.insert_op_load(i32, index);
        let temp = self.get_const_int(32, true, 22);
        let cmp = self.insert_op_s_less_than_equal(bool, loaded_index, temp);
        let selection_merge = self.get_next_id();
        self.insert_op_selection_merge(selection_merge, 0);
        let true_label = self.get_next_id();
        let false_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // int uDelta = 22 - index;
        self.insert_op_label(true_label);
        let loaded_index = self.insert_op_load(i32, index);
        let temp = self.get_const_int(32, true, 22);
        let sub = self.insert_op_i_sub(i32, temp, loaded_index);
        self.insert_op_store(index, sub);

        // return FromFractionAndExp(int(trawValue << uDelta), tExponent - uDelta);
        let loaded_traw_value = self.insert_op_load(i64, traw_value);
        let loaded_u_delta = self.insert_op_load(i32, index);
        let shift = self.insert_op_shift_left_logical(i64, loaded_traw_value, loaded_u_delta);
        let convert = self.insert_op_s_convert(i32, shift);
        self.insert_op_store(param, convert);

        let loaded_exponent = self.insert_op_load(i32, t_exponent);
        let loaded_u_delta = self.insert_op_load(i32, index);
        let sub = self.insert_op_i_sub(i32, loaded_exponent, loaded_u_delta);
        self.insert_op_store(index, sub);

        let call = self.insert_op_function_call(i32, sf32_from_fraction_and_exp, &[param, index]);
        self.insert_op_return_value(call);

        // Else:
        // int uDelta = index - 22;
        self.insert_op_label(false_label);
        let loaded_index = self.insert_op_load(i32, index);
        let temp = self.get_const_int(32, true, 22);
        let sub = self.insert_op_i_sub(i32, loaded_index, temp);
        self.insert_op_store(index, sub);

        // return FromFractionAndExp(int(trawValue >> uDelta), tExponent + uDelta);
        let loaded_traw_value = self.insert_op_load(i64, traw_value);
        let loaded_u_delta = self.insert_op_load(i32, index);
        let shift = self.insert_op_shift_right_arithmetic(i64, loaded_traw_value, loaded_u_delta);
        let convert = self.insert_op_s_convert(i32, shift);
        self.insert_op_store(param, convert);

        let loaded_exponent = self.insert_op_load(i32, t_exponent);
        let loaded_u_delta = self.insert_op_load(i32, index);
        let sub = self.insert_op_i_add(i32, loaded_exponent, loaded_u_delta);
        self.insert_op_store(index, sub);

        let call = self.insert_op_function_call(i32, sf32_from_fraction_and_exp, &[param, index]);
        self.insert_op_return_value(call);

        // End:
        self.insert_op_label(selection_merge);
        let undef = self.insert_op_undef(i32);
        self.insert_op_return_value(undef);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn sf32_div(&mut self) -> u32 {
        let name = "sf32_div".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let sf32_get_fraction = self.sf32_get_fraction();
        let sf32_get_exponent = self.sf32_get_exponent();
        let sf32_normalize_64 = self.sf32_normalize_64();

        let bool = self.get_op_type_bool();
        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);
        let i64 = self.get_op_type_int(64, true);
        let pi64 = self.get_pointer_type(i64, 7);

        // OpFunction
        let op_type_function = self.get_type_function(i32, &[pi32, pi32]);
        let id = self.insert_op_function(i32, 0x8, op_type_function);
        let lhs = self.insert_op_function_parameter(pi32);
        let rhs = self.insert_op_function_parameter(pi32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        let temp_i32 = self.insert_op_variable(pi32, 7);
        let temp_i32_2 = self.insert_op_variable(pi32, 7);
        let temp_i64 = self.insert_op_variable(pi64, 7);

        // int nDivid = GetFraction(rhs);
        let loaded_rhs = self.insert_op_load(i32, rhs);
        self.insert_op_store(temp_i32, loaded_rhs);
        let call = self.insert_op_function_call(i32, sf32_get_fraction, &[temp_i32]);
        self.insert_op_store(temp_i32, call);

        // if (nDivid == 0)
        let loaded_n_divid = self.insert_op_load(i32, temp_i32);
        let temp = self.get_const_int(32, true, 0);
        let cmp = self.insert_op_i_equal(bool, loaded_n_divid, temp);
        let false_label = self.get_next_id();
        self.insert_op_selection_merge(false_label, 0);
        let true_label = self.get_next_id();
        self.insert_op_branch_conditional(cmp, true_label, false_label);

        // return 0;
        self.insert_op_label(true_label);
        let temp = self.get_const_int(32, true, 0);
        self.insert_op_return_value(temp);

        // int64_t trawValue = (int64_t(GetFraction(lhs)) << 32) / nDivid;
        self.insert_op_label(false_label);
        let loaded_lhs = self.insert_op_load(i32, lhs);
        self.insert_op_store(temp_i32_2, loaded_lhs);
        let call = self.insert_op_function_call(i32, sf32_get_fraction, &[temp_i32_2]);
        let convert = self.insert_op_s_convert(i64, call);
        let temp = self.get_const_int(64, true, 32);
        let shift = self.insert_op_shift_left_logical(i64, convert, temp);
        let loaded_n_divid = self.insert_op_load(i32, temp_i32);
        let convert = self.insert_op_s_convert(i64, loaded_n_divid);
        let div = self.insert_op_s_div(i64, shift, convert);
        self.insert_op_store(temp_i64, div);

        // int tExponent = GetExponent(lhs) - GetExponent(rhs) + 95;
        let loaded_lhs = self.insert_op_load(i32, lhs);
        self.insert_op_store(temp_i32, loaded_lhs);
        let call = self.insert_op_function_call(i32, sf32_get_exponent, &[temp_i32]);
        let loaded_rhs = self.insert_op_load(i32, rhs);
        self.insert_op_store(temp_i32, loaded_rhs);
        let call2 = self.insert_op_function_call(i32, sf32_get_exponent, &[temp_i32]);
        let sub = self.insert_op_i_sub(i32, call, call2);
        let temp = self.get_const_int(32, true, 95);
        let add = self.insert_op_i_add(i32, sub, temp);
        self.insert_op_store(temp_i32, add);

        // return Normalize64(trawValue, tExponent);
        let call = self.insert_op_function_call(i32, sf32_normalize_64, &[temp_i64, temp_i32]);
        self.insert_op_return_value(call);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }

    fn f32_conformant_div(&mut self) -> u32 {
        let name = "f32_conformant_div".to_owned();
        if let Some(id) = self.created.get(&name) {
            return *id;
        }

        let sf32_from_float = self.sf32_from_float();
        let sf32_div = self.sf32_div();
        let sf32_to_float = self.sf32_to_float();

        let i32 = self.get_op_type_int(32, true);
        let pi32 = self.get_pointer_type(i32, 7);
        let f32 = self.get_op_type_float(32);
        let pf32 = self.get_pointer_type(f32, 7);

        // OpFunction
        let op_type_function = self.get_type_function(f32, &[pf32, pf32]);
        let id = self.insert_op_function(f32, 0x8, op_type_function);
        let lhs = self.insert_op_function_parameter(pf32);
        let rhs = self.insert_op_function_parameter(pf32);
        let temp = self.get_next_id();
        self.insert_op_label(temp);

        let temp_f32 = self.insert_op_variable(pf32, 7);
        let temp_i32 = self.insert_op_variable(pi32, 7);
        let temp_i32_2 = self.insert_op_variable(pi32, 7);

        // return ToFloat(GDiv(FromFloat(lhs), FromFloat(rhs)));
        let loaded_lhs = self.insert_op_load(f32, lhs);
        self.insert_op_store(temp_f32, loaded_lhs);
        let call = self.insert_op_function_call(i32, sf32_from_float, &[temp_f32]);
        self.insert_op_store(temp_i32, call);

        let loaded_rhs = self.insert_op_load(f32, rhs);
        self.insert_op_store(temp_f32, loaded_rhs);
        let call = self.insert_op_function_call(i32, sf32_from_float, &[temp_f32]);
        self.insert_op_store(temp_i32_2, call);

        let call = self.insert_op_function_call(i32, sf32_div, &[temp_i32, temp_i32_2]);
        self.insert_op_store(temp_i32, call);

        let call = self.insert_op_function_call(f32, sf32_to_float, &[temp_i32]);
        self.insert_op_return_value(call);

        self.insert_op_function_end();

        self.created.insert(name, id);
        id
    }
}
