use crate::{Chip8, Display, ExecuteError, KeyCount, STACK_SIZE};
use rand::RngExt;

impl Chip8 {
    // 00E0: CLS
    pub(crate) fn op_00e0(&mut self) {
        self.framebuffer.fill(false);
    }

    //00EE: RET
    pub(crate) const fn op_00ee(&mut self) -> Result<(), ExecuteError> {
        if self.sp == 0 {
            return Err(ExecuteError::StackUnderflow);
        }
        self.sp -= 1;
        self.pc = self.stack[self.sp as usize];
        Ok(())
    }

    // 1nnn: JP addr
    pub(crate) const fn op_1nnn(&mut self, opcode: u16) {
        let addr = opcode & 0x0FFF;
        self.pc = addr;
    }

    // 2nnn: CALL addr
    pub(crate) const fn op_2nnn(&mut self, opcode: u16) -> Result<(), ExecuteError> {
        if self.sp as usize >= STACK_SIZE {
            return Err(ExecuteError::StackOverflow);
        }
        let addr = opcode & 0x0FFF;
        self.stack[self.sp as usize] = self.pc;
        self.sp += 1;
        self.pc = addr;
        Ok(())
    }

    // 3xkk: SE Vx, byte
    pub(crate) const fn op_3xkk(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let byte = (opcode & 0x00FF) as u8;

        if self.registers[vx] == byte {
            self.pc += 2;
        }
    }

    // 4xkk: SNE Vx, byte
    pub(crate) const fn op_4xkk(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let byte = (opcode & 0x00FF) as u8;

        if self.registers[vx] != byte {
            self.pc += 2;
        }
    }

    // 5xy0: SE Vx, Vy
    pub(crate) const fn op_5xy0(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if self.registers[vx] == self.registers[vy] {
            self.pc += 2;
        }
    }

    // 6xkk: LD Vx, byte
    pub(crate) const fn op_6xkk(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let byte = (opcode & 0x00FF) as u8;

        self.registers[vx] = byte;
    }

    // 7xkk: ADD Vx, byte
    pub(crate) const fn op_7xkk(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let byte = (opcode & 0x00FF) as u8;

        self.registers[vx] = self.registers[vx].wrapping_add(byte);
    }

    // 8xy0: LD Vx, Vy
    pub(crate) const fn op_8xy0(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        self.registers[vx] = self.registers[vy];
    }

    // 8xy1: OR Vx, Vy
    pub(crate) const fn op_8xy1(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if self.quirks.vf_reset {
            self.registers[0xF] = 0;
        }

        self.registers[vx] |= self.registers[vy];
    }

    // 8xy2: AND Vx, Vy
    pub(crate) const fn op_8xy2(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if self.quirks.vf_reset {
            self.registers[0xF] = 0;
        }

        self.registers[vx] &= self.registers[vy];
    }

    // 8xy3: XOR Vx, Vy
    pub(crate) const fn op_8xy3(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if self.quirks.vf_reset {
            self.registers[0xF] = 0;
        }

        self.registers[vx] ^= self.registers[vy];
    }

    // 8xy4: ADD Vx, Vy
    pub(crate) const fn op_8xy4(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        let (new_vx, overflowed) = self.registers[vx].overflowing_add(self.registers[vy]);

        self.registers[vx] = new_vx;
        self.registers[0xF] = if overflowed { 1 } else { 0 }
    }

    // 8xy5: SUB Vx, Vy
    pub(crate) const fn op_8xy5(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        let (new_vx, overflowed) = self.registers[vx].overflowing_sub(self.registers[vy]);

        self.registers[vx] = new_vx;
        self.registers[0xF] = if overflowed { 0 } else { 1 };
    }

    // 8xy6: SHR Vx
    pub(crate) const fn op_8xy6(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if !self.quirks.shifting {
            self.registers[vx] = self.registers[vy];
        }

        let new_vf = self.registers[vx] & 0x1;

        self.registers[vx] >>= 1;
        self.registers[0xF] = new_vf;
    }

    // 8xy7: SUBN Vx, Vy
    pub(crate) const fn op_8xy7(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        let (new_vx, overflowed) = self.registers[vy].overflowing_sub(self.registers[vx]);

        self.registers[vx] = new_vx;
        self.registers[0xF] = if overflowed { 0 } else { 1 };
    }

    // 8xyE: SHL Vx {, Vy}
    pub(crate) const fn op_8xye(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if !self.quirks.shifting {
            self.registers[vx] = self.registers[vy];
        }

        let new_vf = (self.registers[vx] >> 7) & 0x1;

        self.registers[vx] <<= 1;
        self.registers[0xF] = new_vf;
    }

    // 9xy0: SNE Vx, Vy
    pub(crate) const fn op_9xy0(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;

        if self.registers[vx] != self.registers[vy] {
            self.pc += 2;
        }
    }

    // Annn: LD I, addr
    pub(crate) const fn op_annn(&mut self, opcode: u16) {
        let addr = opcode & 0x0FFF;
        self.index = addr;
    }

    // Bnnn: JP V0, addr
    pub(crate) const fn op_bnnn(&mut self, opcode: u16) {
        let addr = opcode & 0x0FFF;
        if self.quirks.jumping {
            let vx = ((opcode & 0x0F00) >> 8) as usize;
            self.pc = addr + self.registers[vx] as u16;
        } else {
            self.pc = self.registers[0] as u16 + addr;
        }
    }

    // Cxkk: RND Vx, byte
    pub(crate) fn op_cxkk(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let byte = (opcode & 0x00FF) as u8;

        let rand_byte: u8 = rand::rng().random();

        self.registers[vx] = rand_byte & byte;
    }

    // Dxyn: DRW Vx, Vy, nibble
    pub(crate) fn op_dxyn(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let vy = ((opcode & 0x00F0) >> 4) as usize;
        let height = (opcode & 0x000F) as u8;

        let x_pos = self.registers[vx] % Display::WIDTH_U8;
        let y_pos = self.registers[vy] % Display::HEIGHT_U8;

        let mut flipped = false;

        for row in 0..height {
            let sprite_byte = self.memory[(self.index + row as u16) as usize];

            if self.quirks.clipping && (y_pos + row) >= Display::HEIGHT_U8 {
                break;
            }

            for col in 0..8 {
                if self.quirks.clipping && (x_pos + col) >= Display::WIDTH_U8 {
                    break;
                }

                if (sprite_byte & (0x80 >> col)) != 0 {
                    let x_pos = ((x_pos + col) % Display::WIDTH_U8) as usize;
                    let y_pos = ((y_pos + row) % Display::HEIGHT_U8) as usize;
                    let idx = x_pos + Display::WIDTH_USIZE * y_pos;

                    flipped |= self.framebuffer[idx];
                    self.framebuffer[idx] ^= true;
                }
            }
        }
        self.registers[0xF] = flipped as u8;
    }

    // Ex9E: SKP vx
    pub(crate) const fn op_ex9e(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        let key = self.registers[vx] as usize;

        if self.keys[key] {
            self.pc += 2;
        }
    }

    // ExA1: SKNP Vx
    pub(crate) const fn op_exa1(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        let key = self.registers[vx] as usize;

        if !self.keys[key] {
            self.pc += 2;
        }
    }

    // Fx07: LD Vx, DT
    pub(crate) const fn op_fx07(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        self.registers[vx] = self.delay_timer;
    }

    // Fx0A: LD Vx, K
    pub(crate) fn op_fx0a(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        let mut done = false;

        if !self.quirks.release || self.pressed_key.is_none() {
            for i in 0..KeyCount::U8 {
                if self.keys[i as usize] {
                    self.registers[vx] = i;
                    if !self.quirks.release {
                        done = true;
                    }
                    self.pressed_key = Some(i as usize);
                    break;
                }
            }
        }

        if self.quirks.release && self.pressed_key.is_some_and(|val| !self.keys[val]) {
            self.pressed_key = None;
            done = true;
        }

        if !done {
            self.pc -= 2;
        }
    }

    // Fx15: LD DT, Vx
    pub(crate) const fn op_fx15(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        self.delay_timer = self.registers[vx];
    }

    // Fx18: LD ST, Vx
    pub(crate) const fn op_fx18(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        self.sound_timer = self.registers[vx];
    }

    // Fx1E: ADD I, Vx
    pub(crate) const fn op_fx1e(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;

        self.index = self.index.wrapping_add(self.registers[vx] as u16);
    }

    // Fx29: LD F, Vx
    pub(crate) const fn op_fx29(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let digit = self.registers[vx] as u16;

        self.index = digit * 5;
    }

    // Fx33: LD B, Vx
    pub(crate) const fn op_fx33(&mut self, opcode: u16) {
        let vx = ((opcode & 0x0F00) >> 8) as usize;
        let mut value = self.registers[vx];

        // Ones-place
        self.memory[(self.index + 2) as usize] = value % 10;
        value /= 10;

        // Tens-place
        self.memory[(self.index + 1) as usize] = value % 10;
        value /= 10;

        // Hundreds-place
        self.memory[self.index as usize] = value % 10;
    }

    // Fx55: LD [I], Vx
    pub(crate) fn op_fx55(&mut self, opcode: u16) {
        let vx = (opcode & 0x0F00) >> 8;

        for i in 0..=vx {
            self.memory[(self.index + i) as usize] = self.registers[i as usize];
        }

        if self.quirks.memory {
            self.index = self.index + vx + 1;
        }
    }

    // Fx65: LD Vx, [I]
    pub(crate) fn op_fx65(&mut self, opcode: u16) {
        let vx = (opcode & 0x0F00) >> 8;

        for i in 0..=vx {
            self.registers[i as usize] = self.memory[(self.index + i) as usize];
        }

        if self.quirks.memory {
            self.index = self.index + vx + 1;
        }
    }
}
