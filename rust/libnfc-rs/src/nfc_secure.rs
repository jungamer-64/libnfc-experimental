use libc::{c_char, c_int, size_t};
use std::ptr;
use std::sync::atomic::{compiler_fence, Ordering};

#[cfg(feature = "nfc_secure_debug")]
use std::ffi::CStr;

const NFC_SECURE_SUCCESS: c_int = 0;
const NFC_SECURE_ERROR_INVALID: c_int = -1;
const NFC_SECURE_ERROR_OVERFLOW: c_int = -2;
const NFC_SECURE_ERROR_RANGE: c_int = -3;
const NFC_SECURE_ERROR_ZERO_SIZE: c_int = -4;

fn validate_params(dst: *mut u8, dst_size: size_t, src: *const u8, src_size: size_t) -> c_int {
    if dst.is_null() || src.is_null() {
        return NFC_SECURE_ERROR_INVALID;
    }
    if src_size == 0 {
        return NFC_SECURE_SUCCESS;
    }
    let max = (size_t::MAX) / 2;
    if src_size > max || dst_size > max {
        return NFC_SECURE_ERROR_RANGE;
    }
    if dst_size < src_size {
        return NFC_SECURE_ERROR_OVERFLOW;
    }
    NFC_SECURE_SUCCESS
}

#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_safe_memcpy(
    dst: *mut libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int(|| {
        let res = validate_params(dst as *mut u8, dst_size, src as *const u8, src_size);
        if res != NFC_SECURE_SUCCESS {
            return res;
        }
        if src_size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, src_size);
        NFC_SECURE_SUCCESS
    })
}

#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_safe_memmove(
    dst: *mut libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int(|| {
        let res = validate_params(dst as *mut u8, dst_size, src as *const u8, src_size);
        if res != NFC_SECURE_SUCCESS {
            return res;
        }
        if src_size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        ptr::copy(src as *const u8, dst as *mut u8, src_size);
        NFC_SECURE_SUCCESS
    })
}

#[must_use = "Return value must be checked for errors"]
#[no_mangle]
pub unsafe extern "C" fn nfc_secure_memset(
    ptr: *mut libc::c_void,
    val: libc::c_int,
    size: size_t,
) -> c_int {
    crate::ffi_catch_unwind_int(|| {
        if ptr.is_null() {
            return NFC_SECURE_ERROR_INVALID;
        }
        if size == 0 {
            return NFC_SECURE_SUCCESS;
        }
        let max = (size_t::MAX) / 2;
        if size > max {
            return NFC_SECURE_ERROR_RANGE;
        }
        // Platform-specific secure zeroing when available. Instead of using
        // early returns inside cfg blocks (which can cause unreachable-code
        // warnings when one of the cfgs is active), set a flag and return
        // once we've used a platform API.
        let done = {
            let mut d = false;

            #[cfg(have_secure_zero_memory)]
            {
                extern "system" {
                    fn SecureZeroMemory(ptr: *mut libc::c_void, cnt: libc::size_t);
                }
                unsafe { SecureZeroMemory(ptr as *mut _, size as libc::size_t) };
                d = true;
            }

            #[cfg(have_explicit_bzero)]
            {
                extern "C" {
                    fn explicit_bzero(s: *mut libc::c_void, n: libc::size_t);
                }
                unsafe { explicit_bzero(ptr as *mut _, size as libc::size_t) };
                d = true;
            }

            #[cfg(have_memset_s)]
            {
                extern "C" {
                    fn memset_s(
                        dest: *mut libc::c_void,
                        destsz: libc::size_t,
                        ch: libc::c_int,
                        count: libc::size_t,
                    ) -> libc::c_int;
                }
                let res = unsafe {
                    memset_s(
                        ptr as *mut _,
                        size as libc::size_t,
                        val as libc::c_int,
                        size as libc::size_t,
                    )
                };
                if res != 0 {
                    crate::log_error("nfc_secure_memset: memset_s failed");
                    return NFC_SECURE_ERROR_INVALID;
                }
                d = true;
            }

            d
        };

        if done {
            return NFC_SECURE_SUCCESS;
        }

        // Fallback: choose algorithm depending on buffer size
        const NFC_SECURE_MEMSET_THRESHOLD: usize = 256;
        let len = size as usize;
        let dst = ptr as *mut u8;

        if len <= NFC_SECURE_MEMSET_THRESHOLD {
            // Small buffers: volatile byte-wise write
            for i in 0..len {
                unsafe { ptr::write_volatile(dst.add(i), val as u8) };
            }
            return NFC_SECURE_SUCCESS;
        }

        // Large buffers: use libc::memset for speed, then ensure the write is not optimized away
        unsafe {
            libc::memset(ptr, val as libc::c_int, len);
        }
        compiler_fence(Ordering::SeqCst);
        unsafe { ptr::write_volatile(dst, val as u8) };
        NFC_SECURE_SUCCESS
    })
}

#[no_mangle]

pub extern "C" fn nfc_secure_strerror(code: c_int) -> *const c_char {
    match code {
        NFC_SECURE_SUCCESS => b"Success\0".as_ptr() as *const c_char,
        NFC_SECURE_ERROR_INVALID => {
            b"Invalid parameter (NULL pointer or invalid input)\0".as_ptr() as *const c_char
        }
        NFC_SECURE_ERROR_OVERFLOW => {
            b"Buffer overflow prevented (destination too small)\0".as_ptr() as *const c_char
        }
        NFC_SECURE_ERROR_RANGE => b"Size parameter out of valid range\0".as_ptr() as *const c_char,
        NFC_SECURE_ERROR_ZERO_SIZE => {
            b"Zero-size operation (deprecated, now treated as success)\0".as_ptr() as *const c_char
        }
        _ => b"Unknown error code\0".as_ptr() as *const c_char,
    }
}

#[no_mangle]
pub unsafe extern "C" fn nfc_safe_strlen(str: *const c_char, maxlen: size_t) -> size_t {
    if str.is_null() {
        return 0;
    }
    let mut len: usize = 0;
    while len < (maxlen as usize) {
        let b = *(str.add(len) as *const u8);
        if b == 0 {
            break;
        }
        len += 1;
    }
    len as size_t
}

#[no_mangle]
pub unsafe extern "C" fn nfc_is_null_terminated(buf: *const c_char, bufsize: size_t) -> c_int {
    if buf.is_null() || bufsize == 0 {
        return 0;
    }
    let mut i: usize = 0;
    while i < (bufsize as usize) {
        if *buf.add(i) as u8 == 0 {
            return 1;
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn nfc_ensure_null_terminated(buf: *mut c_char, bufsize: size_t) {
    if buf.is_null() || bufsize == 0 {
        return;
    }
    let mut found_null = false;
    let mut i: usize = 0;
    while i < (bufsize as usize) {
        if *buf.add(i) as u8 == 0 {
            found_null = true;
            break;
        }
        i += 1;
    }
    if !found_null {
        // Overwrite last byte with NUL
        *buf.add(bufsize as usize - 1) = 0;
    }
}

#[cfg(feature = "nfc_secure_debug")]
#[no_mangle]
pub unsafe extern "C" fn nfc_buffers_overlap(
    dst: *const libc::c_void,
    dst_size: size_t,
    src: *const libc::c_void,
    src_size: size_t,
) -> c_int {
    if dst.is_null() || src.is_null() {
        return 0;
    }
    let dst_ptr = dst as usize;
    let src_ptr = src as usize;
    let dst_len = dst_size as usize;
    let src_len = src_size as usize;

    if dst_ptr >= src_ptr && dst_ptr < src_ptr + src_len {
        return 1;
    }
    if src_ptr >= dst_ptr && src_ptr < dst_ptr + dst_len {
        return 1;
    }
    0
}

#[cfg(feature = "nfc_secure_debug")]
#[no_mangle]
pub unsafe extern "C" fn nfc_check_suspicious_size(dst_size: size_t, func_name: *const c_char) {
    // Helper: small utility to detect power-of-two sizes
    fn is_power_of_2(n: usize) -> bool {
        n != 0 && (n & (n - 1)) == 0
    }
    // Heuristic: if dst_size equals pointer size and is small (<=16), warn
    let ptr_size = std::mem::size_of::<*const libc::c_void>();
    let sz = dst_size as usize;
    if (sz == ptr_size && sz <= 16) || (is_power_of_2(sz) && sz <= 16) {
        let func = if func_name.is_null() {
            "<unknown>"
        } else {
            match CStr::from_ptr(func_name).to_str() {
                Ok(s) => s,
                Err(_) => "<non-utf8>",
            }
        };
        let msg = format!(
            "{}: WARNING - dst_size={} matches pointer size ({} bytes). Did you pass a pointer instead of an array?",
            func, sz, ptr_size
        );
        // Use the crate-level logging helper
        crate::log_error(&msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn memcpy_success() {
        unsafe {
            let mut dst = [0u8; 8];
            let src = [1u8, 2, 3, 4];
            let rc = nfc_safe_memcpy(
                dst.as_mut_ptr() as *mut _,
                dst.len(),
                src.as_ptr() as *const _,
                src.len(),
            );
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            assert_eq!(&dst[..4], &src);
        }
    }

    #[test]
    fn memcpy_overflow() {
        unsafe {
            let mut dst = [0u8; 2];
            let src = [1u8, 2, 3, 4];
            let rc = nfc_safe_memcpy(
                dst.as_mut_ptr() as *mut _,
                dst.len(),
                src.as_ptr() as *const _,
                src.len(),
            );
            assert_eq!(rc, NFC_SECURE_ERROR_OVERFLOW);
        }
    }

    #[test]
    fn memset_zero() {
        unsafe {
            let mut buf = [0xFFu8; 4];
            let rc = nfc_secure_memset(buf.as_mut_ptr() as *mut _, 0, buf.len());
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            assert_eq!(buf, [0u8; 4]);
        }
    }

    #[test]
    fn strlen_null_and_bounds() {
        unsafe {
            // NULL pointer returns 0
            assert_eq!(nfc_safe_strlen(std::ptr::null(), 10), 0);

            let s = CString::new("hello").unwrap();
            // normal case
            assert_eq!(nfc_safe_strlen(s.as_ptr(), 100) as usize, 5);
            // maxlen smaller than actual length
            assert_eq!(nfc_safe_strlen(s.as_ptr(), 3) as usize, 3);

            // buffer without NUL in the first N bytes
            let v = vec![b'A'; 6];
            let p = v.as_ptr() as *const c_char;
            assert_eq!(nfc_safe_strlen(p, 6) as usize, 6);
        }
    }

    #[test]
    fn null_terminated_helpers() {
        unsafe {
            // is_null_terminated: NULL -> 0
            assert_eq!(nfc_is_null_terminated(std::ptr::null(), 10), 0);

            // buffer with NUL in range (create bytes with interior NUL)
            let inner = vec![b'a', b'b', 0u8, b'c', b'd'];
            let p_inner = inner.as_ptr() as *const c_char;
            assert_eq!(nfc_is_null_terminated(p_inner, 5), 1);

            // buffer without NUL in first N
            let mut v = vec![b'X'; 4];
            let p = v.as_mut_ptr() as *mut c_char;
            assert_eq!(nfc_is_null_terminated(p as *const c_char, 4), 0);

            // ensure_null_terminated modifies last byte
            nfc_ensure_null_terminated(p, 4);
            assert_eq!(*p.add(3) as u8, 0);

            // already terminated case: should leave existing terminator
            let mut buf = [b'A', b'\0', b'B'];
            let pb = buf.as_mut_ptr() as *mut c_char;
            nfc_ensure_null_terminated(pb, 3);
            assert_eq!(buf[1], 0);
        }
    }

    #[cfg(feature = "nfc_secure_debug")]
    #[test]
    fn buffers_overlap_detects_overlap() {
        unsafe {
            let mut a = [0u8; 8];
            let pa = a.as_mut_ptr() as *mut libc::c_void;
            // overlapping: dst starts at a[2], src at a[0]
            let dst = pa.add(2) as *const libc::c_void;
            let src = pa as *const libc::c_void;
            assert_eq!(nfc_buffers_overlap(dst, 4, src, 4), 1);

            // non-overlap
            let mut b = [0u8; 8];
            let pb = b.as_mut_ptr() as *const libc::c_void;
            assert_eq!(nfc_buffers_overlap(pb, 4, pb.add(4), 4), 0);
        }
    }

    #[cfg(feature = "nfc_secure_debug")]
    #[test]
    fn suspicious_size_logs_warning() {
        unsafe {
            crate::test_clear_last_log();
            let psz = std::mem::size_of::<*const libc::c_void>();
            let name = CString::new("check_test").unwrap();
            nfc_check_suspicious_size(psz as size_t, name.as_ptr());
            let logged = crate::test_get_last_log();
            assert!(logged.is_some());
            assert!(logged.unwrap().contains("WARNING - dst_size="));
        }
    }

    #[test]
    fn memset_large_zeroes_buffer() {
        unsafe {
            let mut buf = vec![0xFFu8; 512];
            let p = buf.as_mut_ptr() as *mut libc::c_void;
            let rc = nfc_secure_memset(p, 0, buf.len());
            assert_eq!(rc, NFC_SECURE_SUCCESS);
            for &b in &buf {
                assert_eq!(b, 0);
            }
        }
    }

    #[test]
    fn memset_null_ptr_returns_invalid() {
        unsafe {
            let rc = nfc_secure_memset(std::ptr::null_mut(), 0, 10);
            assert_eq!(rc, NFC_SECURE_ERROR_INVALID);
        }
    }

    #[test]
    fn memset_size_range_checks() {
        unsafe {
            // Very large size should be rejected
            let mut buf = vec![0u8; 8];
            // Use a size greater than SIZE_MAX/2 simulated by using a huge usize (truncate on 64-bit)
            let large = (usize::MAX / 2) + 100usize;
            let rc = nfc_secure_memset(buf.as_mut_ptr() as *mut _, 0, large);
            assert_eq!(rc, NFC_SECURE_ERROR_RANGE);
        }
    }
}
