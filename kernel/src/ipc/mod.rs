pub mod pipe;

#[allow(unused_imports)]
pub use pipe::{Pipe, PipeReadError, PipeWriteError, PIPE_CAPACITY};

pub fn run_ipc_tests() -> bool {
    let mut pipe = Pipe::new();

    // 1. Basic Read/Write
    let msg = b"Hello from IPC Pipe!";
    match pipe.write(1, msg) {
        Ok(n) if n == msg.len() => {}
        _ => return false,
    }

    let mut read_buf = [0u8; 32];
    match pipe.read(2, &mut read_buf) {
        Ok(n) if n == msg.len() && &read_buf[..n] == msg => {}
        _ => return false,
    }

    // 2. Ordering
    let m1 = b"Part1;";
    let m2 = b"Part2;";
    if pipe.write(1, m1).unwrap_or(0) != m1.len() {
        return false;
    }
    if pipe.write(1, m2).unwrap_or(0) != m2.len() {
        return false;
    }

    let mut order_buf = [0u8; 32];
    let n = pipe.read(2, &mut order_buf).unwrap_or(0);
    if n != 12 || &order_buf[..12] != b"Part1;Part2;" {
        return false;
    }

    // 3. Blocking when empty with active writers
    let mut empty_buf = [0u8; 16];
    match pipe.read(2, &mut empty_buf) {
        Err(PipeReadError::WouldBlock) => {}
        _ => return false,
    }

    // Blocked reader list should have PID 2
    if !pipe.blocked_readers.contains(&2) {
        return false;
    }

    // Writing wakes reader
    if pipe.write(1, b"Wakeup").is_err() {
        return false;
    }
    let to_wake = pipe.take_readers_to_wake();
    if !to_wake.contains(&2) {
        return false;
    }

    // Read remaining data
    let mut wake_buf = [0u8; 16];
    let _ = pipe.read(2, &mut wake_buf);

    // 4. EOF on closed writer
    let (is_empty, to_wake) = pipe.close_write();
    if is_empty || !to_wake.is_empty() {
        // readers still exist (1 reader), writers = 0
    }
    match pipe.read(2, &mut empty_buf) {
        Ok(0) => {} // EOF
        _ => return false,
    }

    // 5. Broken Pipe on closed reader
    let mut pipe2 = Pipe::new();
    let (_, wake_writers) = pipe2.close_read();
    if !wake_writers.is_empty() {
        return false;
    }
    match pipe2.write(1, b"NoReader") {
        Err(PipeWriteError::BrokenPipe) => {}
        _ => return false,
    }

    true
}
