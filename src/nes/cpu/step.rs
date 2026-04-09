//! Result types for single-instruction CPU execution.

/// Successful execution of one CPU instruction.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StepOk {
    /// Normal instruction — caller advances PC by `size` bytes.
    Advance {
        /// Instruction length in bytes (1–3).
        size: u8,
        /// CPU cycles consumed.
        cycles: u8,
    },
    /// Branch or jump — the handler already set PC.
    Jump {
        /// CPU cycles consumed.
        cycles: u8,
    },
}

impl StepOk {
    /// Returns the number of CPU cycles consumed.
    pub(crate) fn cycles(self) -> u8 {
        match self {
            Self::Advance { cycles, .. } | Self::Jump { cycles } => cycles,
        }
    }
}

/// Failure or halt during instruction execution.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StepErr {
    /// BRK instruction encountered.
    Break,
    /// Unrecognized or illegal opcode.
    IllegalOpcode(u8),
}

/// Result of executing one CPU step.
pub(crate) type StepResult = Result<StepOk, StepErr>;
