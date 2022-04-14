/// An error that can be thrown on a memory access.
#[derive(Debug)]
pub enum BusError {
    /// The address is invalid (but well-aligned).
    AccessFault,
    /// The address is misaligned.
    AddressMisaligned,
}

/// A bus facilitating accesses on the emulated physical bus.
pub trait Bus<A, V> {
    /// Loads the value located at the given `address`.
    fn load(&self, address: A) -> Result<V, BusError>;
    /// Stores the given `value` to the given `address`.
    fn store(&self, address: A, value: V) -> Result<(), BusError>;
}

/// A bus facilitating accesses to the emulated CSRs.
pub trait Csr {
    /// Attempts to access the CSR at the given `address` by setting the value
    /// of the CSR to the result of calling `f` with its current value.
    ///
    /// Returns the original value of the CSR on success.
    ///
    /// Returns `Err(CsrIllegal)` when the access is illegal for some reason,
    /// usually because the CSR does not exist or because the current privilege
    /// level does not have access.
    fn access(
        &mut self,
        address: CsrAddress,
        f: impl FnOnce(u64) -> u64,
    ) -> Result<u64, CsrIllegal>;
}

/// Returned by [Csr::access] to indicate an illegal CSR access.
pub struct CsrIllegal;

/// A wrapper for integers guaranteed to be less than 4096 that is used to represent
/// CSR addresses.
#[derive(Clone, Copy)]
pub struct CsrAddress(u16);

impl Csr for () {
    fn access(&mut self, address: CsrAddress, f: impl FnOnce(u64) -> u64) -> Result<u64, CsrIllegal> {
        Err(CsrIllegal)
    }
}

impl CsrAddress {
    /// Creates a new CsrAddress if the given address is less than 4096.
    pub const fn new(address: u16) -> Option<CsrAddress> {
        if address < 4096 {
            Some(CsrAddress(address))
        } else {
            None
        }
    }

    /// Creates a new CsrAddress and assumes that the given address is less than 4096.
    /// 
    /// # Safety
    /// `address` must be less than 4096. If it is not, undefined behavior may be invoked,
    /// because users of this type are permitted to assume that the interior address is
    /// less than 4096. 
    pub const unsafe fn new_unchecked(address: u16) -> CsrAddress {
        CsrAddress(address)
    }

    /// Gets the actual address.
    pub const fn address(self) -> u16 {
        self.0
    }
}
