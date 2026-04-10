// BerkeOS — bexvm.rs
// BerkeBex Virtual Machine - Execute .bex bytecode programs

const MAX_BYTECODE_SIZE: usize = 65536;
const MAX_CALL_DEPTH: usize = 64;
const MAX_FUNC_COUNT: usize = 256;
const MAX_ARRAYS: usize = 32;
const MAX_ARRAY_LEN: usize = 64;
const MAX_DICTS: usize = 32;
const MAX_DICT_ENTRIES: usize = 32;
const MAX_EXCEPTION_DEPTH: usize = 16;

#[derive(Clone, Copy)]
struct BexArray {
    data: [i64; MAX_ARRAY_LEN],
    len: usize,
}

impl BexArray {
    fn new() -> Self {
        Self {
            data: [0; MAX_ARRAY_LEN],
            len: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct BexDictEntry {
    key: [u8; 64],
    key_len: usize,
    value: i64,
}

impl BexDictEntry {
    fn new() -> Self {
        Self {
            key: [0; 64],
            key_len: 0,
            value: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct BexDict {
    entries: [BexDictEntry; MAX_DICT_ENTRIES],
    len: usize,
}

impl BexDict {
    fn new() -> Self {
        Self {
            entries: [BexDictEntry::new(); MAX_DICT_ENTRIES],
            len: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct ExceptionFrame {
    handler_ip: usize,
    stack_snapshot: usize,
}

pub struct BexVM {
    stack: [i64; 256],
    sp: usize,
    locals: [i64; 32],
    call_stack: [(usize, [i64; 32]); MAX_CALL_DEPTH],
    call_sp: usize,
    arrays: [BexArray; MAX_ARRAYS],
    array_next: usize,
    dicts: [BexDict; MAX_DICTS],
    dict_next: usize,
    exception_stack: [Option<ExceptionFrame>; MAX_EXCEPTION_DEPTH],
    exception_sp: usize,
    current_exception: Option<i64>,
}

impl BexVM {
    pub fn new() -> Self {
        Self {
            stack: [0; 256],
            sp: 0,
            locals: [0; 32],
            call_stack: [(0, [0; 32]); MAX_CALL_DEPTH],
            call_sp: 0,
            arrays: [BexArray::new(); MAX_ARRAYS],
            array_next: 0,
            dicts: [BexDict::new(); MAX_DICTS],
            dict_next: 0,
            exception_stack: [None; MAX_EXCEPTION_DEPTH],
            exception_sp: 0,
            current_exception: None,
        }
    }

    fn push(&mut self, val: i64) {
        if self.sp < 256 {
            self.stack[self.sp] = val;
            self.sp += 1;
        }
    }

    fn pop(&mut self) -> i64 {
        if self.sp > 0 {
            self.sp -= 1;
            self.stack[self.sp]
        } else {
            0
        }
    }

    fn print_str(&self, s: &[u8]) {
        for &b in s {
            crate::drivers::serial::write_byte(b);
        }
    }

    fn print_i64(&self, mut val: i64) {
        if val == 0 {
            crate::drivers::serial::write_byte(b'0');
            return;
        }
        let neg = val < 0;
        if neg {
            val = -val;
        }
        let mut buf = [0u8; 32];
        let mut idx = 0usize;
        while val > 0 {
            buf[idx] = b'0' + (val % 10) as u8;
            val /= 10;
            idx += 1;
        }
        if neg {
            crate::drivers::serial::write_byte(b'-');
        }
        for i in (0..idx).rev() {
            crate::drivers::serial::write_byte(buf[i]);
        }
    }

    fn print_const(&self, data: &[u8], offset: &mut usize, name_len: usize) {
        let tag = data[*offset];
        *offset += 1;
        match tag {
            1 => {
                let val = i32::from_le_bytes([
                    data[*offset],
                    data[*offset + 1],
                    data[*offset + 2],
                    data[*offset + 3],
                ]);
                *offset += 4;
                self.print_i64(val as i64);
            }
            2 => {
                let val = f64::from_le_bytes([
                    data[*offset],
                    data[*offset + 1],
                    data[*offset + 2],
                    data[*offset + 3],
                    data[*offset + 4],
                    data[*offset + 5],
                    data[*offset + 6],
                    data[*offset + 7],
                ]);
                *offset += 8;
                self.print_i64(val as i64);
                crate::drivers::serial::write_byte(b'.');
                crate::drivers::serial::write_byte(b'0');
            }
            3 => {
                let len = u32::from_le_bytes([
                    data[*offset],
                    data[*offset + 1],
                    data[*offset + 2],
                    data[*offset + 3],
                ]) as usize;
                *offset += 4;
                for i in 0..len {
                    crate::drivers::serial::write_byte(data[*offset + i]);
                }
                *offset += len;
            }
            4 => {
                let b = data[*offset] != 0;
                *offset += 1;
                if b {
                    self.print_str(b"true");
                } else {
                    self.print_str(b"false");
                }
            }
            5 => {
                let count = u32::from_le_bytes([
                    data[*offset],
                    data[*offset + 1],
                    data[*offset + 2],
                    data[*offset + 3],
                ]) as usize;
                *offset += 4;
                self.print_str(b"{");
                for i in 0..count {
                    if i > 0 {
                        self.print_str(b", ");
                    }
                    let key_len = u32::from_le_bytes([
                        data[*offset],
                        data[*offset + 1],
                        data[*offset + 2],
                        data[*offset + 3],
                    ]) as usize;
                    *offset += 4;
                    for j in 0..key_len {
                        crate::drivers::serial::write_byte(data[*offset + j]);
                    }
                    *offset += key_len;
                    self.print_str(b": ");
                    let val_idx = u32::from_le_bytes([
                        data[*offset],
                        data[*offset + 1],
                        data[*offset + 2],
                        data[*offset + 3],
                    ]) as usize;
                    *offset += 4;
                    let mut search_off = 8 + name_len + 4;
                    for _ in 0..val_idx {
                        if search_off >= data.len() {
                            break;
                        }
                        let t = data[search_off];
                        search_off += 1;
                        match t {
                            1 => search_off += 4,
                            2 => search_off += 8,
                            3 => {
                                let l = u32::from_le_bytes([
                                    data[search_off],
                                    data[search_off + 1],
                                    data[search_off + 2],
                                    data[search_off + 3],
                                ]) as usize;
                                search_off += 4 + l;
                            }
                            4 => search_off += 1,
                            5 => {
                                let cnt = u32::from_le_bytes([
                                    data[search_off],
                                    data[search_off + 1],
                                    data[search_off + 2],
                                    data[search_off + 3],
                                ]) as usize;
                                search_off += 4;
                                for _ in 0..cnt {
                                    let kl = u32::from_le_bytes([
                                        data[search_off],
                                        data[search_off + 1],
                                        data[search_off + 2],
                                        data[search_off + 3],
                                    ]) as usize;
                                    search_off += 4 + kl + 4;
                                }
                            }
                            _ => break,
                        }
                    }
                    if search_off < data.len() {
                        self.print_const(data, &mut search_off, name_len);
                    }
                }
                self.print_str(b"}");
            }
            _ => {}
        }
    }

    pub fn run(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() < 8 {
            return Err("File too small");
        }
        if data.len() > MAX_BYTECODE_SIZE {
            return Err("File too large");
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != 0x42455831 {
            return Err("Not .bex");
        }
        let version = u16::from_le_bytes([data[4], data[5]]);
        if version != 1 {
            return Err("Unsupported version");
        }
        let name_len = u16::from_le_bytes([data[6], data[7]]) as usize;

        crate::drivers::serial::write_str("\r\n[BERUN] Running: ");
        for i in 0..name_len {
            crate::drivers::serial::write_byte(data[8 + i]);
        }
        crate::drivers::serial::write_str("\r\n\r\n");

        let mut offset = 8 + name_len;
        let const_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        for _ in 0..const_count {
            if offset >= data.len() {
                break;
            }
            let tag = data[offset];
            offset += 1;
            match tag {
                1 => offset += 4,
                2 => offset += 8,
                3 => {
                    let len = u32::from_le_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]) as usize;
                    offset += 4 + len;
                }
                4 => offset += 1,
                5 => {
                    let count = u32::from_le_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]) as usize;
                    offset += 4;
                    for _ in 0..count {
                        let key_len = u32::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ]) as usize;
                        offset += 4 + key_len + 4;
                    }
                }
                _ => break,
            }
        }

        let func_count = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        let mut func_addrs = [0usize; MAX_FUNC_COUNT];
        let func_count = func_count.min(MAX_FUNC_COUNT);
        for i in 0..func_count {
            if offset >= data.len() {
                break;
            }
            func_addrs[i] = offset;
            let fn_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2 + fn_len + 4;
            let instr_count = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4 + (instr_count * 5);
        }

        let mut ip = 0usize;
        let code_start = offset;

        loop {
            let pos = code_start + ip;
            if pos >= data.len() {
                break;
            }
            let opcode = data[pos];
            ip += 1;

            match opcode {
                0 => {} // NOP
                1 => {
                    // PUSH
                    let idx = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]) as usize;
                    ip += 4;
                    let const_off = 8 + name_len + 4;
                    let mut off = const_off;
                    for _ in 0..idx {
                        if off >= data.len() {
                            break;
                        }
                        let t = data[off];
                        off += 1;
                        match t {
                            1 => off += 4,
                            2 => off += 8,
                            3 => {
                                let l = u32::from_le_bytes([
                                    data[off],
                                    data[off + 1],
                                    data[off + 2],
                                    data[off + 3],
                                ]) as usize;
                                off += 4 + l;
                            }
                            4 => off += 1,
                            5 => {
                                let count = u32::from_le_bytes([
                                    data[off],
                                    data[off + 1],
                                    data[off + 2],
                                    data[off + 3],
                                ]) as usize;
                                off += 4;
                                for _ in 0..count {
                                    let kl = u32::from_le_bytes([
                                        data[off],
                                        data[off + 1],
                                        data[off + 2],
                                        data[off + 3],
                                    ]) as usize;
                                    off += 4 + kl + 4;
                                }
                            }
                            _ => break,
                        }
                    }
                    if off < data.len() {
                        self.print_const(data, &mut off, name_len);
                    }
                }
                2 => {
                    let index = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]) as usize;
                    if index < 32 {
                        self.push(self.locals[index]);
                    }
                } // PUSHLOCAL
                3 => {
                    let index = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]) as usize;
                    let v = self.pop();
                    if index < 32 {
                        self.locals[index] = v;
                    }
                } // STORELOCAL
                4 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a + b);
                } // ADD
                5 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a - b);
                } // SUB
                6 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(a * b);
                } // MUL
                7 => {
                    let b = self.pop();
                    let a = self.pop();
                    if b != 0 {
                        self.push(a / b);
                    }
                } // DIV
                8 => {
                    let b = self.pop();
                    let a = self.pop();
                    if b != 0 {
                        self.push(a % b);
                    }
                } // MOD
                9 => {
                    let v = self.pop();
                    self.push(-v);
                } // NEGATE
                10 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push(if a == b { 1 } else { 0 });
                } // CMP
                11 => {
                    let target = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]);
                    ip = target as usize;
                } // JMP
                12 => {
                    let target = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]);
                    ip += 4;
                    if self.pop() == 0 {
                        ip = target as usize;
                    }
                } // JIF
                13 => {
                    let _ = self.pop();
                } // PRINT
                14 => {
                    let _ = self.pop();
                    crate::drivers::serial::write_str("\r\n");
                } // PRINTLN
                15 => {
                    if self.call_sp > 0 {
                        self.call_sp -= 1;
                        let (ret_ip, saved_locals) = self.call_stack[self.call_sp];
                        ip = ret_ip;
                        self.locals = saved_locals;
                    } else {
                        break;
                    }
                } // RET
                16 => {
                    break;
                } // HALT
                17 => {
                    let _ = self.pop();
                } // POP
                18 => {
                    let syscall_id = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]);
                    ip += 4;
                    match syscall_id {
                        // new_array() - creates new array, returns array ref
                        0 => {
                            if self.array_next < MAX_ARRAYS {
                                let arr_idx = self.array_next;
                                self.arrays[arr_idx] = BexArray::new();
                                self.array_next += 1;
                                self.push(arr_idx as i64);
                            } else {
                                self.push(-1); // error: no space
                            }
                        }
                        // len() - takes array ref, returns length
                        1 => {
                            let arr_idx = self.pop() as usize;
                            if arr_idx < self.array_next {
                                self.push(self.arrays[arr_idx].len as i64);
                            } else {
                                self.push(0);
                            }
                        }
                        // push() - takes value and array ref, appends value to array
                        2 => {
                            let val = self.pop();
                            let arr_idx = self.pop() as usize;
                            if arr_idx < self.array_next && self.arrays[arr_idx].len < MAX_ARRAY_LEN
                            {
                                self.arrays[arr_idx].data[self.arrays[arr_idx].len] = val;
                                self.arrays[arr_idx].len += 1;
                            }
                        }
                        _ => {}
                    }
                }
                19 => {
                    let val = self.stack[self.sp - 1];
                    self.push(val);
                }
                20 => {
                    self.push(0);
                }
                21 => {
                    let _ms = self.pop();
                }
                22 => {
                    use core::num::Wrapping;
                    static mut SEED: u64 = 12345;
                    unsafe {
                        SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
                        let val = ((SEED >> 16) & 0x7FFF) as i64;
                        self.push(val);
                    }
                }
                23 => {
                    let func_idx = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]) as usize;
                    ip += 4;
                    if func_idx < func_count && self.call_sp < MAX_CALL_DEPTH {
                        let mut saved_locals = self.locals;
                        core::mem::swap(&mut saved_locals, &mut self.locals);
                        self.call_stack[self.call_sp] = (ip, saved_locals);
                        self.call_sp += 1;
                        ip = func_addrs[func_idx];
                    }
                }
                24 => {
                    let handler_ip = i32::from_le_bytes([
                        data[pos + 1],
                        data[pos + 2],
                        data[pos + 3],
                        data[pos + 4],
                    ]) as usize;
                    ip += 4;
                    if self.exception_sp < MAX_EXCEPTION_DEPTH {
                        self.exception_stack[self.exception_sp] = Some(ExceptionFrame {
                            handler_ip,
                            stack_snapshot: self.sp,
                        });
                        self.exception_sp += 1;
                    }
                }
                25 => {
                    if self.exception_sp > 0 {
                        self.exception_sp -= 1;
                        if let Some(frame) = self.exception_stack[self.exception_sp] {
                            self.current_exception = Some(self.pop());
                            self.sp = frame.stack_snapshot;
                            ip = frame.handler_ip;
                        }
                    }
                }
                26 => {
                    let msg_ptr = self.pop() as usize;
                    let exc_type = self.pop();
                    self.current_exception = Some((exc_type << 32) | (msg_ptr as i64));
                    if self.exception_sp > 0 {
                        self.exception_sp -= 1;
                        if let Some(frame) = self.exception_stack[self.exception_sp] {
                            self.sp = frame.stack_snapshot;
                            ip = frame.handler_ip;
                        }
                    }
                }
                27 => {
                    let idx = self.pop() as usize;
                    let arr_ref = self.pop() as usize;
                    if arr_ref < self.array_next && idx < self.arrays[arr_ref].len {
                        self.push(self.arrays[arr_ref].data[idx]);
                    } else {
                        self.push(0);
                    }
                }
                28 => {
                    let val = self.pop();
                    let idx = self.pop() as usize;
                    let arr_ref = self.pop() as usize;
                    if arr_ref < self.array_next && idx < MAX_ARRAY_LEN {
                        self.arrays[arr_ref].data[idx] = val;
                    }
                }
                29 => {
                    let dict_ref = self.pop() as usize;
                    let key_idx = self.pop() as usize;
                    let key_off = 8 + name_len + 4;
                    let mut off = key_off;
                    for _ in 0..dict_ref {
                        if off >= data.len() {
                            break;
                        }
                        let t = data[off];
                        off += 1;
                        match t {
                            1 => off += 4,
                            2 => off += 8,
                            3 => {
                                let l = u32::from_le_bytes([
                                    data[off],
                                    data[off + 1],
                                    data[off + 2],
                                    data[off + 3],
                                ]) as usize;
                                off += 4 + l;
                            }
                            4 => off += 1,
                            5 => {
                                let cnt = u32::from_le_bytes([
                                    data[off],
                                    data[off + 1],
                                    data[off + 2],
                                    data[off + 3],
                                ]) as usize;
                                off += 4;
                                for _ in 0..cnt {
                                    let kl = u32::from_le_bytes([
                                        data[off],
                                        data[off + 1],
                                        data[off + 2],
                                        data[off + 3],
                                    ]) as usize;
                                    off += 4 + kl + 4;
                                }
                            }
                            _ => break,
                        }
                    }
                    if off < data.len() && data[off] == 5 {
                        off += 1;
                        let count = u32::from_le_bytes([
                            data[off],
                            data[off + 1],
                            data[off + 2],
                            data[off + 3],
                        ]) as usize;
                        off += 4;
                        let mut found = false;
                        for _ in 0..count {
                            let key_len = u32::from_le_bytes([
                                data[off],
                                data[off + 1],
                                data[off + 2],
                                data[off + 3],
                            ]) as usize;
                            off += 4;
                            let mut key_bytes = [0u8; 64];
                            for i in 0..key_len.min(64) {
                                key_bytes[i] = data[off + i];
                            }
                            off += key_len;
                            let val_idx = u32::from_le_bytes([
                                data[off],
                                data[off + 1],
                                data[off + 2],
                                data[off + 3],
                            ]) as usize;
                            off += 4;
                            if key_len < 64 && dict_ref == key_idx as usize {
                                let mut lookup_off = key_off;
                                for _ in 0..val_idx {
                                    if lookup_off >= data.len() {
                                        break;
                                    }
                                    let lt = data[lookup_off];
                                    lookup_off += 1;
                                    match lt {
                                        1 => lookup_off += 4,
                                        2 => lookup_off += 8,
                                        3 => {
                                            let ll = u32::from_le_bytes([
                                                data[lookup_off],
                                                data[lookup_off + 1],
                                                data[lookup_off + 2],
                                                data[lookup_off + 3],
                                            ])
                                                as usize;
                                            lookup_off += 4 + ll;
                                        }
                                        4 => lookup_off += 1,
                                        5 => {
                                            let lcnt = u32::from_le_bytes([
                                                data[lookup_off],
                                                data[lookup_off + 1],
                                                data[lookup_off + 2],
                                                data[lookup_off + 3],
                                            ])
                                                as usize;
                                            lookup_off += 4;
                                            for _ in 0..lcnt {
                                                let lkl = u32::from_le_bytes([
                                                    data[lookup_off],
                                                    data[lookup_off + 1],
                                                    data[lookup_off + 2],
                                                    data[lookup_off + 3],
                                                ])
                                                    as usize;
                                                lookup_off += 4 + lkl + 4;
                                            }
                                        }
                                        _ => break,
                                    }
                                }
                                if lookup_off < data.len() {
                                    let tag = data[lookup_off];
                                    lookup_off += 1;
                                    match tag {
                                        1 => {
                                            let val = i32::from_le_bytes([
                                                data[lookup_off],
                                                data[lookup_off + 1],
                                                data[lookup_off + 2],
                                                data[lookup_off + 3],
                                            ]);
                                            self.push(val as i64);
                                        }
                                        4 => {
                                            self.push(if data[lookup_off] != 0 { 1 } else { 0 });
                                        }
                                        _ => self.push(0),
                                    }
                                    found = true;
                                    break;
                                }
                            }
                        }
                        if !found {
                            self.push(0);
                        }
                    } else {
                        self.push(0);
                    }
                }
                30 => {
                    let b = self.pop();
                    let a = self.pop();
                    self.push((a << 32) | (b & 0xFFFFFFFF));
                }
                _ => {}
            }
        }
        crate::drivers::serial::write_str("\r\n[BERUN] Done.\r\n");
        Ok(())
    }
}

impl Default for BexVM {
    fn default() -> Self {
        Self::new()
    }
}

pub fn run_bex_file(data: &[u8]) -> Result<(), &'static str> {
    let mut vm = BexVM::new();
    vm.run(data)
}
