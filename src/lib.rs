//! A platform agnostic Rust driver for the ST7032i, based on the
//! [`embedded-hal`](https://github.com/japaric/embedded-hal) traits.
//!
//! ## The Device
//!
//! The Sitronix ST7032i is a dot matrix LCD controller with I²C interface.
//!
//! - [Details and datasheet](http://www.newhavendisplay.com/app_notes/ST7032.pdf)
//!
//! ## Usage
//!
//! ### Instantiating
//!
//! Import this crate and an `embedded_hal` implementation:
//!
//! ```
//! extern crate linux_embedded_hal as hal;
//! extern crate st7032i;
//! ```
//!
//! Then instantiate the device:
//!
//! ```no_run
//! # extern crate linux_embedded_hal as hal;
//! # extern crate st7032i;
//! use hal::{Delay, I2cdev};
//! use st7032i::ST7032i;
//!
//! # fn main() {
//! let dev = I2cdev::new("/dev/i2c-1")?;
//! let mut display = ST7032i::new(dev, Delay, 2);
//! display.init()?;
//! writeln!(display, "Hello")?;
//! display.move_cursor(1, 0)?;
//! writeln!(display, "Rust")?;
//! # }
//! ```

#![no_std]

extern crate embedded_hal as hal;

use core::fmt;
use hal::blocking::delay::DelayMs;
use hal::blocking::i2c::{Read, Write, WriteRead};

pub const I2C_ADRESS: u8 = 0x3e;

/// ST7032i instruction set
#[derive(Debug, PartialEq)]
enum InstructionSet {
    Normal,
    Extented,
}

/// Text moving direction
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Direction {
    LeftToRigh,
    RightToLeft,
}

/// Driver for the ST7032i
#[derive(Debug)]
pub struct ST7032i<I2C, D> {
    i2c: I2C,
    delay: D,
    entry: Direction,
    lines: u8,
    scroll: bool,
    display: bool,
    cursor: bool,
    blink: bool,
}

impl<I2C, E, D> ST7032i<I2C, D>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
    D: DelayMs<u8>,
{
    /// Initialize the ST7032i driver.
    pub fn new(i2c: I2C, delay: D, lines: u8) -> Self {
        ST7032i {
            i2c,
            delay,
            lines,
            entry: Direction::RightToLeft,
            scroll: false,
            display: false,
            cursor: false,
            blink: false,
        }
    }

    /// Initialize the display.
    pub fn init(&mut self) -> Result<(), E> {
        match self.send_function(InstructionSet::Normal, 1, false) {
            Ok(_) => self.delay.delay_ms(1),
            Err(_) => self.delay.delay_ms(20),
        };

        self.send_function(InstructionSet::Extented, 1, false)?;
        self.delay.delay_ms(5);

        self.send_function(InstructionSet::Extented, 1, false)?;
        self.delay.delay_ms(5);

        self.send_function(InstructionSet::Extented, self.lines, false)?;
        self.delay.delay_ms(5);

        self.off()?;

        self.send_osc_config(true, 0)?;
        self.send_contrast(0)?;
        self.send_booster_config(true, false, 0)?;
        self.send_follower_config(true, 0)?;

        self.send_entry_mode()?;
        self.delay.delay_ms(20);

        self.on()?;

        self.clear()
    }

    /// Switch display on
    pub fn on(&mut self) -> Result<(), E> {
        self.display = true;
        self.send_display_mode()
    }

    /// Switch display off
    pub fn off(&mut self) -> Result<(), E> {
        self.display = false;
        self.send_display_mode()
    }

    /// Clear all the display data by writing "20H" (space code)
    /// to all DDRAM address, and set DDRAM address to "00H" into AC (address counter).
    pub fn clear(&mut self) -> Result<(), E> {
        const CLEAR_DISPLAY: u8 = 0b_00000001;
        self.send_command(CLEAR_DISPLAY)?;
        self.delay.delay_ms(2);
        Ok(())
    }

    /// Set DDRAM address to "0" and return cursor to its original position if shifted.
    /// The contents of DDRAM are not changed.
    pub fn home(&mut self) -> Result<(), E> {
        const RETURN_HOME: u8 = 0b_00000010;
        self.send_command(RETURN_HOME)?;
        self.delay.delay_ms(2);
        Ok(())
    }

    /// Move cursor to specified location
    pub fn move_cursor(&mut self, row: u8, col: u8) -> Result<(), E> {
        let command = match row {
            0 => col | 0b_10000000,
            _ => col | 0b_11000000,
        };
        self.send_command(command)
    }

    /// Show cursor
    pub fn show_cursor(&mut self, blink: bool) -> Result<(), E> {
        self.cursor = true;
        self.blink = blink;
        self.send_display_mode()
    }

    /// Hide cursor
    pub fn hide_cursor(&mut self) -> Result<(), E> {
        self.cursor = false;
        self.blink = false;
        self.send_display_mode()
    }

    /// Enable autoscroll
    pub fn enable_scroll(&mut self, entry: Direction) -> Result<(), E> {
        self.scroll = true;
        self.entry = entry;
        self.send_entry_mode()
    }

    /// Disable autoscroll
    pub fn disable_scroll(&mut self) -> Result<(), E> {
        self.scroll = false;
        self.send_entry_mode()
    }

    /// Shift display to specified direction
    pub fn shift_display(&mut self, dir: Direction) -> Result<(), E> {
        let mut command = 0b_00011000;
        if dir == Direction::LeftToRigh {
            command |= 0b_00000100;
        }
        self.send_command(command)
    }

    /// Shift cursor to specified direction
    pub fn shift_cursor(&mut self, dir: Direction) -> Result<(), E> {
        let mut command = 0b_00010000;
        if dir == Direction::LeftToRigh {
            command |= 0b_00000100;
        }
        self.send_command(command)
    }

    fn send_entry_mode(&mut self) -> Result<(), E> {
        let mut command = 0b_00000100;
        if self.scroll {
            command |= 0b_00000001;
        }
        if self.entry == Direction::LeftToRigh {
            command |= 0b_00000010;
        }
        self.send_command(command)
    }

    fn send_display_mode(&mut self) -> Result<(), E> {
        let mut command = 0b_00001000;
        if self.blink {
            command |= 0b_00000001;
        }
        if self.cursor {
            command |= 0b_00000010;
        }
        if self.display {
            command |= 0b_00000100;
        }
        self.send_command(command)
    }

    fn send_function(&mut self, is: InstructionSet, lines: u8, dbl: bool) -> Result<(), E> {
        let mut command = 0b_00110000;
        if lines > 1 {
            command |= 0b_00001000;
        } else if dbl {
            command |= 0b_00000100;
        }
        if is == InstructionSet::Extented {
            command |= 0b_00000001;
        }
        self.send_command(command)
    }

    fn send_osc_config(&mut self, bias: bool, freq: u8) -> Result<(), E> {
        assert!(freq < 8);
        let mut command = 0b_00010000 | freq;
        if bias {
            command |= 0b_00001000;
        }
        self.send_command(command)
    }

    fn send_contrast(&mut self, contrast: u8) -> Result<(), E> {
        assert!(contrast < 16);
        self.send_command(0b_01110000 | contrast)
    }

    fn send_booster_config(&mut self, on: bool, icon: bool, contrast_low: u8) -> Result<(), E> {
        assert!(contrast_low < 4);
        let mut command = 0b_01010000 | contrast_low;
        if on {
            command |= 0b_00000100;
        }
        if icon {
            command |= 0b_00001000;
        }
        self.send_command(command)
    }

    fn send_follower_config(&mut self, on: bool, ratio: u8) -> Result<(), E> {
        assert!(ratio < 8);
        let mut command = 0b_01100000 | ratio;
        if on {
            command |= 0b_00001000;
        }
        self.send_command(command)
    }

    fn send_command(&mut self, command: u8) -> Result<(), E> {
        self.i2c.write(I2C_ADRESS, &[0b_00000000, command])?;
        self.delay.delay_ms(1);
        Ok(())
    }
}

impl<I2C, E, D> fmt::Write for ST7032i<I2C, D>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
    D: DelayMs<u8>,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            self.i2c.write(I2C_ADRESS, &[0b_01000000, *byte]).ok();
        }
        Ok(())
    }
}
