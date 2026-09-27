#![allow(clippy::cast_lossless)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::option_if_let_else)]

mod instructions;

const MEMORY_SIZE: usize = 4096;
const REGISTER_COUNT: usize = 16;
const STACK_SIZE: usize = 16;
const FONT_SET_SIZE: usize = 80;

pub struct Display;

impl Display {
    const RAW_WIDTH: u8 = 64;
    const RAW_HEIGHT: u8 = 32;

    pub const WIDTH_U8: u8 = Self::RAW_WIDTH;
    pub const WIDTH_USIZE: usize = Self::RAW_WIDTH as usize;
    pub const WIDTH_U32: u32 = Self::RAW_WIDTH as u32;
    pub const WIDTH_F32: f32 = Self::RAW_WIDTH as f32;

    pub const HEIGHT_U8: u8 = Self::RAW_HEIGHT;
    pub const HEIGHT_USIZE: usize = Self::RAW_HEIGHT as usize;
    pub const HEIGHT_U32: u32 = Self::RAW_HEIGHT as u32;
    pub const HEIGHT_F32: f32 = Self::RAW_HEIGHT as f32;
}

struct StartAddr;

impl StartAddr {
    pub const U16: u16 = 0x200;
    pub const USIZE: usize = Self::U16 as usize;
}

struct KeyCount;

impl KeyCount {
    pub const U8: u8 = 16;
    pub const USIZE: usize = Self::U8 as usize;
}

const FONT_SET: [u8; FONT_SET_SIZE] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

#[derive(Debug)]
pub struct Chip8 {
    memory: [u8; MEMORY_SIZE],
    registers: [u8; REGISTER_COUNT],
    index: u16,
    pc: u16,
    sp: u8,
    stack: [u16; STACK_SIZE],
    delay_timer: u8,
    sound_timer: u8,
    keys: [bool; KeyCount::USIZE],
    framebuffer: [bool; Display::WIDTH_USIZE * Display::HEIGHT_USIZE],
    quirks: Quirks,
    // Used to check if pressed key is released
    pressed_key: Option<usize>,
}

impl Chip8 {
    #[must_use]
    pub fn new() -> Self {
        let mut memory = [0; MEMORY_SIZE];

        memory[..FONT_SET_SIZE].copy_from_slice(&FONT_SET[..]);

        Self {
            memory,
            registers: [0; REGISTER_COUNT],
            index: 0,
            pc: StartAddr::U16,
            sp: 0,
            stack: [0; STACK_SIZE],
            delay_timer: 0,
            sound_timer: 0,
            keys: [false; KeyCount::USIZE],
            framebuffer: [false; Display::WIDTH_USIZE * Display::HEIGHT_USIZE],
            quirks: Quirks::new(),
            pressed_key: None,
        }
    }

    pub fn reset(&mut self) {
        self.memory = [0; MEMORY_SIZE];
        self.registers = [0; REGISTER_COUNT];
        self.index = 0;
        self.pc = StartAddr::U16;
        self.sp = 0;
        self.stack = [0; STACK_SIZE];
        self.delay_timer = 0;
        self.sound_timer = 0;
        self.keys = [false; KeyCount::USIZE];
        self.framebuffer = [false; Display::WIDTH_USIZE * Display::HEIGHT_USIZE];

        self.memory[..FONT_SET_SIZE].copy_from_slice(&FONT_SET[..]);
    }

    pub fn load_program(&mut self, data: &[u8]) {
        self.memory[StartAddr::USIZE..(StartAddr::USIZE + data.len())].copy_from_slice(data);
    }

    pub fn reload_with_quirks(&mut self, quirks: Quirks, data: &[u8]) {
        self.quirks = quirks;
        self.reset();
        self.load_program(data);
    }

    #[must_use]
    pub const fn framebuffer(&self) -> &[bool] {
        &self.framebuffer
    }

    #[must_use]
    pub const fn quirks(&self) -> Quirks {
        self.quirks
    }

    pub const fn set_quirks(&mut self, quirks: Quirks) {
        self.quirks = quirks;
    }

    pub const fn set_key(&mut self, idx: usize, pressed: bool) {
        self.keys[idx] = pressed;
    }

    /// Runs a single cycle of the emulator by fetching, decoding, and executing an instruction.
    ///
    /// # Errors
    ///
    /// This function will return an [`ExecuteError`] if:
    ///
    /// * **`UndefinedInstruction`** - The fetched 16-bit opcode does not match any valid CHIP-8 instruction.
    /// * **`StackOverflow`** - A subroutine call (`0x2NNN`) is executed, but the call stack is already at maximum capacity.
    /// * **`StackUnderflow`** - A return instruction (`0x00EE`) is executed, but the call stack is already empty.
    pub fn emulate(&mut self) -> Result<(), ExecuteError> {
        // Fetch
        let opcode = self.fetch();

        // Decode and Execute
        self.execute(opcode)?;

        Ok(())
    }

    pub const fn tick_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }

        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    const fn fetch(&mut self) -> u16 {
        let high_byte = self.memory[self.pc as usize] as u16;
        let low_byte = self.memory[(self.pc + 1) as usize] as u16;
        let opcode = (high_byte << 8) | low_byte;
        self.pc += 2;
        opcode
    }

    fn execute(&mut self, opcode: u16) -> Result<(), ExecuteError> {
        match (
            (opcode & 0xF000) >> 12,
            (opcode & 0x0F00) >> 8,
            (opcode & 0x00F0) >> 4,
            opcode & 0x000F,
        ) {
            (0x0, 0x0, 0xE, 0x0) => self.op_00e0(),
            (0x0, 0x0, 0xE, 0xE) => self.op_00ee()?,
            (0x1, _, _, _) => self.op_1nnn(opcode),
            (0x2, _, _, _) => self.op_2nnn(opcode)?,
            (0x3, _, _, _) => self.op_3xkk(opcode),
            (0x4, _, _, _) => self.op_4xkk(opcode),
            (0x5, _, _, _) => self.op_5xy0(opcode),
            (0x6, _, _, _) => self.op_6xkk(opcode),
            (0x7, _, _, _) => self.op_7xkk(opcode),
            (0x8, _, _, 0x0) => self.op_8xy0(opcode),
            (0x8, _, _, 0x1) => self.op_8xy1(opcode),
            (0x8, _, _, 0x2) => self.op_8xy2(opcode),
            (0x8, _, _, 0x3) => self.op_8xy3(opcode),
            (0x8, _, _, 0x4) => self.op_8xy4(opcode),
            (0x8, _, _, 0x5) => self.op_8xy5(opcode),
            (0x8, _, _, 0x6) => self.op_8xy6(opcode),
            (0x8, _, _, 0x7) => self.op_8xy7(opcode),
            (0x8, _, _, 0xE) => self.op_8xye(opcode),
            (0x9, _, _, 0x0) => self.op_9xy0(opcode),
            (0xA, _, _, _) => self.op_annn(opcode),
            (0xB, _, _, _) => self.op_bnnn(opcode),
            (0xC, _, _, _) => self.op_cxkk(opcode),
            (0xD, _, _, _) => self.op_dxyn(opcode),
            (0xE, _, 0x9, 0xE) => self.op_ex9e(opcode),
            (0xE, _, 0xA, 0x1) => self.op_exa1(opcode),
            (0xF, _, 0x0, 0x7) => self.op_fx07(opcode),
            (0xF, _, 0x0, 0xA) => self.op_fx0a(opcode),
            (0xF, _, 0x1, 0x5) => self.op_fx15(opcode),
            (0xF, _, 0x1, 0x8) => self.op_fx18(opcode),
            (0xF, _, 0x1, 0xE) => self.op_fx1e(opcode),
            (0xF, _, 0x2, 0x9) => self.op_fx29(opcode),
            (0xF, _, 0x3, 0x3) => self.op_fx33(opcode),
            (0xF, _, 0x5, 0x5) => self.op_fx55(opcode),
            (0xF, _, 0x6, 0x5) => self.op_fx65(opcode),
            _ => return Err(ExecuteError::UndefinedInstruction(opcode)),
        }
        Ok(())
    }
}

impl Default for Chip8 {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quirks {
    /// The AND, OR and XOR opcodes (`8xy1`, `8xy2` and `8xy3`) reset the flags register to zero.
    pub vf_reset: bool,
    /// The save and load opcodes (`Fx55` and `Fx65`) increment the index register.
    pub memory: bool,
    /// Sprites drawn at the bottom edge of the screen get clipped instead of wrapping around the screen.
    pub clip: bool,
    /// The shift opcodes (`8xy6` and `8xyE`) only operate on vX instead of storing the shifted version of vY in vX.
    pub shift: bool,
    /// The jump instruction (`Bnnn`) doesn't use v0, but vX instead where X is the highest nibble of nnn.
    pub jump: bool,
    /// The get key instruction (`Fx0A`) waits for a key press and key up.
    pub release: bool,
}

impl Default for Quirks {
    fn default() -> Self {
        Self {
            vf_reset: true,
            memory: true,
            clip: true,
            shift: false,
            jump: false,
            release: true,
        }
    }
}

impl Quirks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug)]
pub enum ExecuteError {
    UndefinedInstruction(u16),
    StackOverflow,
    StackUnderflow,
}

impl std::fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UndefinedInstruction(opcode) => write!(f, "Undefined instruction {opcode:#04x}"),
            Self::StackOverflow => write!(f, "Stack overflow"),
            Self::StackUnderflow => write!(f, "Stack underflow"),
        }
    }
}

impl std::error::Error for ExecuteError {}
