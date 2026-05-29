use std::io;
use std::io::Write;
use std::mem::MaybeUninit;

pub struct Display {
	old: libc::termios,
}

impl Display {
	pub fn new() -> io::Result<Self> {
		// could probably use ::uninit() instead of ::zeroed() and be fine
		let mut old = MaybeUninit::<libc::termios>::zeroed();
		let mut err = unsafe {
			libc::tcgetattr(libc::STDIN_FILENO, old.as_mut_ptr())
		};
		if err != 0 {
			return Err(io::Error::last_os_error());
		}
		let old = unsafe { old.assume_init() };
		let mut new = old;
		new.c_lflag &= !(libc::ICANON | libc::ECHO);
		new.c_cc[libc::VMIN] = 1;
		new.c_cc[libc::VTIME] = 0;
		err = unsafe {
			libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &new)
		};
		if err != 0 {
			return Err(io::Error::last_os_error());
		}
		let _ = io::stdout().write_all(b"\x1b[2J\x1b[H");
		let _ = io::stdout().flush();
		Ok(Self {
			old,
		})
	}

	pub fn update(&self, addr: u16, val: u16) {
		// TODO: use top half of val for color or something
		let ch = val as u8 as char;
		let _ = io::stdout().write_fmt(
			format_args!("\x1b[{};{}H{}", addr / 80 + 1, addr % 80 + 1, ch)
		);
		let _ = io::stdout().flush();
	}
}

impl Drop for Display {
	fn drop(&mut self) {
		unsafe {
			libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.old);
		}
	}
}
