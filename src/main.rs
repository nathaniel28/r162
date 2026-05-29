mod dev;

const DISPLAY_MEM_START: u16 = 0x0100;
const DISPLAY_MEM_END: u16 = 0x0880; // up to but not including

fn sign_extend16(bits: u16, val: u16) -> i16 {
	(
		if val & (1 << (bits - 1)) != 0 {
			val | (!0u16 << bits)
		} else {
			val
		}
	) as i16
}

fn imm_si(w: u16) -> u16 {
	(w >> 8) & 0xf
}

fn imm_ls(w: u16) -> u16 {
	(w >> 5) & 0x7
}

fn imm_b(w: u16) -> i16 {
	sign_extend16(8, (w >> 4) & 0xff)
}

fn imm_i(w: u16) -> i16 {
	sign_extend16(9, ((w & 0xff0) | ((w & 0x8) << 9)) >> 4)
}

fn imm_j(w: u16) -> i16 {
	sign_extend16(12, ((w & 0xcff0) | ((w & 0xc) << 10)) >> 4)
}

fn reg_r(w: u16) -> usize {
	((w >> 12) & 0xf) as usize
}

fn reg_b(w: u16) -> usize {
	((w >> 8) & 0xf) as usize
}

fn reg_a(w: u16) -> usize {
	((w >> 4) & 0xf) as usize
}

fn reg_j(w: u16) -> usize {
	((w >> 12) & 0x3) as usize
}

// could align each register
/*
#[repr(align(4))]
struct Aligned<T>(T);
*/

struct State {
	regs: [u16; 16],
	pc: u16,
	imem: Vec<u16>,
	dmem: Vec<u16>,
	display: dev::Display,
}

impl State {
	fn new() -> Self {
		const SIZE: usize = 1 << 16;
		let mut imem = Vec::with_capacity(SIZE);
		imem.resize(SIZE, 0);
		let mut dmem = Vec::with_capacity(SIZE);
		dmem.resize(SIZE, 0);
		let display = dev::Display::new().unwrap(); // hehe
		Self {
			regs: [0; 16],
			pc: 0,
			imem,
			dmem,
			display,
		}
	}

	fn set_reg(&mut self, id: usize, val: u16) {
		if id != 0 {
			self.regs[id] = val;
		}
	}

	fn write_alert(&mut self, addr: u16, val: u16) {
		// check if this is memory mapped to some device
		// and if it is, update that device
		match addr {
			DISPLAY_MEM_START..DISPLAY_MEM_END => {
				self.display.update(addr - DISPLAY_MEM_START, val);
			}
			_ => {}
		}
	}

	fn step(&mut self) {
		let w = self.imem[self.pc as usize];
		self.pc = self.pc.wrapping_add(1);
		match w & 0x3 {
			0x0 => {
				let rr = reg_r(w);
				let a = self.regs[rr];
				match w & 0x18 {
					0x0 => {
						// S*-type
						let b = self.regs[reg_b(w)];
						let i = imm_si(w);
						let res;
						match w & 0xe4 {
							// SR-types ahead
							0x0 => res = a << b, // op:sll
							0x4 => res = ((a as i16) >> (b as i16)) as u16, // op:sra
							0x40 => res = a >> b, // op:srl
							0x44 => res = a ^ b, // op:xor
							0x80 => res = a | b, // op:or
							0x84 => res = a & b, // op:and
							0xc0 => res = (a as i32).wrapping_mul(b as i32) as u16, // op:mul
							0xc4 => res = ((a as i32).wrapping_mul(b as i32) >> 16) as u16, // op:mulh
							0xe0 => {
								// op:div
								let b = if b == 0 { 1 } else { b };
								res = (a as i16).wrapping_div(b as i16) as u16;
							}
							0xe4 => {
								// op:jalr
								res = self.pc;
								self.pc = b;
							}
							// SI-types ahead
							0x20 => res = a << i, // op:slli
							0x24 => res = ((a as i16) >> (i as i16)) as u16, // op:srai
							0x60 => res = a >> i, // op:srli
							0x64 => res = a ^ i, // op:xori
							0xa0 => res = a | i, // op:ori
							0xa4 => res = a & i, // op:andi
							_ => unreachable!()
						}
						self.set_reg(rr, res);
					}
					0x8 | 0x18 => {
						// B-type
						// op:beqz and op:bnez
						if (a != 0) != (w & 0x4 != 0) {
							self.pc = self.pc.wrapping_add_signed(imm_b(w));
						}
					}
					0x10 => {
						// LS-type
						let addr = self.regs[reg_b(w)].wrapping_add(imm_ls(w));
						if w & 0x4 == 0 {
							self.write_alert(addr, self.regs[rr]);
							self.dmem[addr as usize] = self.regs[rr]; // op:sw
						} else {
							self.set_reg(rr, self.dmem[addr as usize]); // op:lw
						}
					}
					_ => unreachable!()
				}
			}
			0x1 => {
				// I-type
				let rr = reg_r(w);
				let mut res = imm_i(w) as u16;
				if w & 0x4 == 0 {
					res = res.wrapping_add(self.regs[rr]); // op:addi
				} else {
					// do nothing for op:li
				}
				self.set_reg(rr, res);
			}
			0x2 => {
				// R-type
				let rr = reg_r(w);
				let b = self.regs[reg_b(w)];
				let a = self.regs[reg_a(w)];
				let res;
				match w & 0xc {
					0x0 => res = a.wrapping_add(b), // op:add
					0x4 => res = a.wrapping_sub(b), // op:sub
					0x8 => res = ((a as i16) < (b as i16)) as u16, // op:slt
					0xc => res = (a < b) as u16, // op:sltu
					_ => unreachable!()
				}
				self.set_reg(rr, res);
			}
			0x3 => {
				// J-type, must be op:jal
				self.set_reg(reg_j(w), self.pc);
				self.pc = self.pc.wrapping_add_signed(imm_j(w));
			}
			_ => unreachable!()
		}
	}
}

fn main() {
	let mut state = State::new();
	state.imem[0] = 0x1005;
	state.imem[1] = 0x2015;
	state.imem[2] = 0x40a5;
	state.imem[3] = 0x3212;
	state.imem[4] = 0x1022;
	state.imem[5] = 0x2032;
	state.imem[6] = 0x4ff9;
	state.imem[7] = 0x4fb8;
	state.imem[8] = 0x2010;
	state.imem[9] = 0xffff;
	for _ in 0..100 {
		state.step();
	}
	use std::io::Write;
    let _ = std::io::stdout().write_fmt(format_args!("\x1b[H0x{:x}", state.dmem[0]));
}
