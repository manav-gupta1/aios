pub fn test_invalid_user_pointer() -> bool {
    // 1. Kernel space pointer must be rejected
    let kernel_ptr = 0xFFFF_8000_0000_0000 as *const u8;
    if crate::memory::validate_user_buffer(kernel_ptr, 32) {
        return false;
    }

    // 2. Kernel heap pointer must be rejected
    let heap_ptr = 0x444444440000 as *const u8;
    if crate::memory::validate_user_buffer(heap_ptr, 16) {
        return false;
    }

    // 3. Null pointer must be rejected
    let null_ptr = core::ptr::null();
    if crate::memory::validate_user_buffer(null_ptr, 10) {
        return false;
    }

    // 4. Overflowing address range must be rejected
    let overflow_ptr = (usize::MAX - 10) as *const u8;
    if crate::memory::validate_user_buffer(overflow_ptr, 20) {
        return false;
    }

    true
}
