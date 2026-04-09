//! NES controller button state as bitflags.
//!
//! Bit order matches the hardware shift register read sequence:
//! A, B, Select, Start, Up, Down, Left, Right.

use bitflags::bitflags;

bitflags! {
    /// Button state for one NES controller.
    ///
    /// Each bit corresponds to one button, in the order the CPU
    /// reads them from the controller's serial shift register.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Buttons: u8 {
        /// A button.
        const A      = 1 << 0;
        /// B button.
        const B      = 1 << 1;
        /// Select button.
        const SELECT = 1 << 2;
        /// Start button.
        const START  = 1 << 3;
        /// D-pad up.
        const UP     = 1 << 4;
        /// D-pad down.
        const DOWN   = 1 << 5;
        /// D-pad left.
        const LEFT   = 1 << 6;
        /// D-pad right.
        const RIGHT  = 1 << 7;
    }
}

/// State of one NES controller port, including the shift register
/// used by the CPU to read buttons serially.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Controller {
    /// Current button state (updated by the frontend each frame).
    buttons: Buttons,
    /// Latched copy of buttons (captured on strobe).
    latch: u8,
    /// Current bit position in the shift register (0–7).
    shift_index: u8,
    /// Whether strobe mode is active (continuously re-latching).
    strobe: bool,
}

impl Controller {
    /// Updates the pressed buttons (called by the frontend).
    pub(crate) fn set_buttons(&mut self, buttons: Buttons) {
        self.buttons = buttons;
        if self.strobe {
            self.latch = self.buttons.bits();
            self.shift_index = 0;
        }
    }

    /// Handles a CPU write to the controller port ($4016).
    ///
    /// Writing 1 enables strobe (continuously re-latches).
    /// Writing 0 disables strobe and freezes the latch.
    pub(super) fn write(&mut self, val: u8) {
        self.strobe = val & 1 != 0;
        if self.strobe {
            self.latch = self.buttons.bits();
            self.shift_index = 0;
        }
    }

    /// Handles a CPU read from the controller port ($4016/$4017).
    ///
    /// Returns the next button bit (LSB), then shifts. After all
    /// 8 bits are read, subsequent reads return 1.
    pub(super) fn read(&mut self) -> u8 {
        if self.strobe {
            return self.buttons.bits() & 1;
        }
        let bit = if self.shift_index < 8 {
            (self.latch >> self.shift_index) & 1
        } else {
            1
        };
        self.shift_index = self.shift_index.saturating_add(1);
        bit
    }
}
