use core::{mem::{size_of, align_of, transmute}, slice, marker::PhantomData};

use crate::bus::{Bus, BusError};

/// An efficient implementation of main memory.
#[repr(transparent)]
pub struct Memory {
    _phantom: PhantomData<*mut u8>,
    data: [u8],
}

impl_bus! {
    u64 u8,
    u64 u16,
    u64 u32,
    u64 u64,
}

impl Memory {
    /// Converts a slice to be used as memory for a hart.
    pub fn new<'d>(data: &'d mut [u64]) -> &'d mut Memory {
        let ptr = data.as_mut_ptr() as *mut u8;
        let len = data.len() * size_of::<u64>();

        // Drop `data` so that there are never mutable references to the same
        // memory at the same time.
        drop(data);

        // SAFETY: The previous slice has been dropped, so we know that this
        // mutable reference is not an alias of another mutable reference.
        let slice = unsafe { slice::from_raw_parts_mut(ptr, len) };

        // SAFETY: We are transmuting between reference types; the inner types
        // have the same memory layout, and the lifetime is the same.
        unsafe { transmute::<&'d mut [u8], &'d mut Memory>(slice) }
    }

    /// Gets the size of memory in bytes.
    pub const fn size(&self) -> usize {
        self.data.len()
    }

    /// Gets a reference to the inner data. You cannot get an immutable reference,
    /// since &[u8] is Send+Sync, while &Memory is not. In other words, an immutable
    /// reference to the slice obtained through some other way than this method
    /// would invoke undefined behavior.
    pub fn data(&mut self) -> &mut [u8] {
        &mut self.data
    }

    #[inline]
    fn calculate_destination<T>(&self, address: usize) -> Result<*mut T, BusError> {
        let upper = address.wrapping_add(size_of::<T>());

        if address < upper && upper <= self.size() {
            // SAFETY: We know that `address` < `self.size()`. Also, we know that
            // `self.size()` < `isize::MAX`, since this is a requirement on the
            // length of slices. Finally, we know it lies within the same allocated
            // object and does not wrap around per the condition of the enclosing if statement.
            let ptr = unsafe { self.data.as_ptr().add(address) } as *mut T;

            if ptr as usize % align_of::<T>() == 0 {
                Ok(ptr)
            } else {
                Err(BusError::AddressMisaligned)
            }
        } else {
            Err(BusError::AccessFault)
        }
    }
}

macro_rules! impl_bus {
    ($($addr:ident $val:ident,)*) => {
        $(impl Bus<$addr, $val> for Memory {
            fn load(&self, address: $addr) -> Result<$val, BusError> {
                if address as usize as u64 == address {
                    let ptr = self.calculate_destination(address as usize)?;
                    
                    // SAFETY: check_address returns a pointer that is guaranteed to be valid.
                    Ok(unsafe { *ptr })
                } else {
                    Err(BusError::AccessFault)
                }
            }

            fn store(&self, address: $addr, value: $val) -> Result<(), BusError> {
                if address as usize as u64 == address {
                    let ptr = self.calculate_destination(address as usize)?;
                    
                    // SAFETY: check_address returns a pointer that is guaranteed to be valid.
                    Ok(unsafe { *ptr = value })
                } else {
                    Err(BusError::AccessFault)
                }
            }
        })*
    };
}

use impl_bus;
