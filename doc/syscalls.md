# BerkeOS System Calls

## Overview

BerkeOS provides a system call interface for user programs (primarily BexVM bytecode) to access kernel services. System calls are invoked via the `syscall` instruction with parameters passed in registers.

## System Call Interface

### Calling Convention

```rust
// Assembly-level syscall invocation
mov rax, syscall_number    // System call number
mov rdi, arg0             // First argument
mov rsi, arg1             // Second argument
mov rdx, arg2             // Third argument
syscall                   // Invoke kernel

// Return value in rax, error in rdx
```

### Return Values

```rust
pub struct SyscallResult {
    pub value: i64,    // Return value or -1 on error
    pub error: i64,    // Error code (0 = success)
}
```

## Process Management Syscalls

### Process Control

- **SYS_EXIT (0)**: Terminate current process
  - `arg0`: Exit code
  - Returns: Never returns

- **SYS_GETPID (3)**: Get current process ID
  - Returns: Current PID

- **SYS_YIELD (5)**: Voluntary context switch
  - Returns: 0

- **SYS_SLEEP (4)**: Sleep for specified time
  - `arg0`: Sleep duration in seconds
  - Returns: 0

## File System Syscalls

### File Operations

- **SYS_FOPEN (12)**: Open/create file
  - `arg0`: Path pointer
  - `arg1`: Path length
  - Returns: File descriptor or error

- **SYS_FREAD (13)**: Read from file
  - `arg0`: File descriptor
  - `arg1`: Buffer pointer
  - `arg2`: Buffer length
  - Returns: Bytes read

- **SYS_FWRITE (14)**: Write to file
  - `arg0`: File descriptor
  - `arg1`: Buffer pointer
  - `arg2`: Buffer length
  - Returns: Bytes written

- **SYS_FCLOSE (15)**: Close file
  - `arg0`: File descriptor
  - Returns: 0

- **SYS_FSEEK (16)**: Seek in file
  - `arg0`: File descriptor
  - `arg1`: Offset
  - Returns: New position

- **SYS_FTELL (17)**: Get file position
  - `arg0`: File descriptor
  - Returns: Current position

### Directory Operations

- **SYS_MKDIR2 (18)**: Create directory
  - `arg0`: Path pointer
  - `arg1`: Path length
  - Returns: 0 or error

- **SYS_DELETE (19)**: Delete file/directory
  - `arg0`: Path pointer
  - `arg1`: Path length
  - Returns: 0 or error

- **SYS_RENAME (20)**: Rename file/directory
  - `arg0`: Old path pointer
  - `arg1`: Old path length
  - `arg2`: New path pointer
  - `arg3`: New path length
  - Returns: 0 or error

- **SYS_EXISTS (21)**: Check if path exists
  - `arg0`: Path pointer
  - `arg1`: Path length
  - Returns: 1 if exists, 0 if not

## Graphics Syscalls

### Framebuffer Operations

- **SYS_FB_INIT (30)**: Initialize framebuffer
  - Returns: 0

- **SYS_FB_PIXEL (31)**: Set pixel color
  - `arg0`: X coordinate
  - `arg1`: Y coordinate
  - `arg2`: RGB color
  - Returns: 0

- **SYS_FB_RECT (32)**: Draw filled rectangle
  - `arg0`: X coordinate
  - `arg1`: Y coordinate
  - `arg2`: Width
  - `arg3`: Height
  - `arg4`: RGB color
  - Returns: 0

- **SYS_FB_CLEAR (33)**: Clear framebuffer
  - `arg0`: RGB color
  - Returns: 0

- **SYS_FB_TEXT (34)**: Draw text
  - `arg0`: X coordinate
  - `arg1`: Y coordinate
  - `arg2`: Text pointer
  - `arg3`: Text length
  - Returns: 0

- **SYS_FB_WIDTH (35)**: Get framebuffer width
  - Returns: Width in pixels

- **SYS_FB_HEIGHT (36)**: Get framebuffer height
  - Returns: Height in pixels

## Input Syscalls

### Keyboard Input

- **SYS_READ_KEY (40)**: Read keyboard scancode
  - Returns: Scancode byte

- **SYS_KEY_DOWN (41)**: Check if key is pressed
  - `arg0`: Key code
  - Returns: 1 if down, 0 if up

## Terminal Syscalls

### Text Mode Operations

- **SYS_TTY_CLEAR (50)**: Clear text screen
  - Returns: 0

- **SYS_TTY_GOTO (51)**: Move cursor
  - `arg0`: X position
  - `arg1`: Y position
  - Returns: 0

- **SYS_TTY_COLOR (52)**: Set text color
  - `arg0`: Foreground color
  - `arg1`: Background color
  - Returns: 0

## Legacy Syscalls

### POSIX Compatibility

- **SYS_WRITE (1)**: Write to stdout/stderr
  - `arg0`: File descriptor (1=stdout, 2=stderr)
  - `arg1`: Buffer pointer
  - `arg2`: Buffer length
  - Returns: Bytes written

- **SYS_READ (2)**: Read from stdin
  - `arg0`: File descriptor (0=stdin)
  - `arg1`: Buffer pointer
  - `arg2`: Buffer length
  - Returns: Bytes read

- **SYS_OPEN (6)**: Open file (legacy)
- **SYS_CLOSE (7)**: Close file (legacy)
- **SYS_STAT (8)**: Get file status (legacy)
- **SYS_MKDIR (9)**: Create directory (legacy)
- **SYS_UNLINK (10)**: Unlink file (legacy)

## System Information

- **SYS_UPTIME (11)**: Get system uptime
  - Returns: Uptime in ticks

## GUI Syscalls (Experimental)

### Window Management

- **SYS_WINDOW_NEW (60)**: Create window
- **SYS_WINDOW_DRAW (61)**: Draw window
- **SYS_BUTTON_NEW (62)**: Create button
- **SYS_LABEL_NEW (63)**: Create label
- **SYS_INPUT_NEW (64)**: Create input field

## Error Codes

```rust
pub const ENOENT: i64 = 2;    // No such file or directory
pub const EBADF: i64 = 9;     // Bad file descriptor
pub const ENOMEM: i64 = 12;   // Out of memory
pub const EINVAL: i64 = 22;   // Invalid argument
pub const ENOSYS: i64 = 38;   // Function not implemented
```

## Implementation

### Syscall Dispatch

```rust
pub fn dispatch(num: u64, arg0: u64, arg1: u64, arg2: u64) -> SyscallResult {
    match num {
        SYS_GETPID => {
            let pid = crate::process::scheduler::current_pid();
            SyscallResult::ok(pid as i64)
        }
        // ... other syscalls
        _ => SyscallResult::err(ENOSYS),
    }
}
```

### Security Considerations

- **Parameter Validation**: All pointer and length arguments validated
- **Bounds Checking**: Array accesses bounds-checked
- **Privilege Checking**: No user/kernel mode distinction yet
- **Resource Limits**: File operations limited to safe sizes

## Future Extensions

- **Network Syscalls**: Socket operations
- **Process Management**: fork, exec, wait
- **Memory Management**: mmap, munmap
- **Signal Handling**: signal, kill
- **Timer Syscalls**: alarm, gettimeofday

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.