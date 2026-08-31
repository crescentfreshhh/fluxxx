//! Per-user credential encryption using Windows DPAPI (`CryptProtectData` /
//! `CryptUnprotectData`). Ciphertext is bound to the current Windows user account
//! and machine, so a copied database file can't be decrypted elsewhere.
//!
//! On non-Windows targets (never shipped — the app builds only on Windows) the
//! functions fall back to an identity transform so the crate still type-checks.

/// Encrypt plaintext bytes for storage at rest.
pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    imp::encrypt(plaintext)
}

/// Decrypt bytes previously produced by [`encrypt`].
pub fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    imp::decrypt(ciphertext)
}

#[cfg(windows)]
mod imp {
    use std::ptr;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    // DPAPI allocates its output buffer with LocalAlloc, so it must be released
    // with LocalFree (kernel32, always linked on Windows). Declared directly to
    // avoid depending on its exact module path in windows-sys.
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hmem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    /// Copy an output DPAPI blob into an owned `Vec` and release the OS buffer.
    unsafe fn take_blob(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let slice = std::slice::from_raw_parts(out.pbData, out.cbData as usize);
        let owned = slice.to_vec();
        LocalFree(out.pbData as *mut core::ffi::c_void);
        owned
    }

    fn in_blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let input = in_blob(plaintext);
            let mut out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: ptr::null_mut(),
            };
            let ok = CryptProtectData(
                &input,
                ptr::null(),      // description
                ptr::null(),      // optional entropy
                ptr::null(),      // reserved
                ptr::null(),      // prompt struct
                0,                // flags
                &mut out,
            );
            if ok == 0 {
                return Err("CryptProtectData failed".to_string());
            }
            Ok(take_blob(out))
        }
    }

    pub fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        unsafe {
            let input = in_blob(ciphertext);
            let mut out = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: ptr::null_mut(),
            };
            let ok = CryptUnprotectData(
                &input,
                ptr::null_mut(),  // description out (ignored)
                ptr::null(),      // optional entropy
                ptr::null(),      // reserved
                ptr::null(),      // prompt struct
                0,                // flags
                &mut out,
            );
            if ok == 0 {
                return Err("CryptUnprotectData failed".to_string());
            }
            Ok(take_blob(out))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(plaintext.to_vec())
    }
    pub fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(ciphertext.to_vec())
    }
}
