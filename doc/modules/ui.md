# User Interface Module

## Overview

The UI module provides BerkeOS's user interface components including the interactive shell (berkesh), text editor (deno), and popup system.

## Berkesh Shell

### Features

- **Command Line Interface**: Interactive CLI with command history
- **30+ Built-in Commands**: File operations, system utilities, games
- **Tab Completion**: Command and filename completion
- **Color Output**: Syntax highlighting and status indicators
- **Multi-drive Support**: Access to 12 BerkeFS drives (Alpha-Mu)

### Shell Architecture

```rust
pub struct Shell {
    // Command line state
    line: [u8; MAX_LINE],
    line_len: usize,
    cursor: usize,
    
    // History system
    history: [[u8; MAX_LINE]; MAX_HISTORY],
    history_len: usize,
    history_idx: usize,
    
    // Display state
    rows: [[u8; COLS]; MAX_ROWS],
    row_count: usize,
    scroll_offset: usize,
    
    // Drive access
    drives: [*mut Mutex<BerkeFS>; 12],
}
```

### Built-in Commands

#### File Operations
- `ls/dir` - List directory contents
- `cat` - Display file contents
- `touch` - Create empty file
- `cp` - Copy files
- `mv` - Move/rename files
- `rm` - Remove files
- `mkdir` - Create directories

#### System Commands
- `ver` - Show version information
- `mem` - Display memory usage
- `sysinfo` - System information
- `date` - Current date/time
- `uptime` - System uptime
- `neofetch` - System info display

#### Tools
- `calc` - Calculator
- `beep` - PC speaker beep
- `play` - Play melodies
- `deno` - Launch text editor
- `format` - Format drives
- `fsck` - Filesystem check

### Command Processing

```rust
fn process_command(&mut self, cmd: &[u8]) {
    // Parse command and arguments
    // Execute appropriate handler
    // Display results
}
```

## Deno Text Editor

### Features

- **Full-screen Editor**: Modal text editing
- **Basic Operations**: Insert, delete, navigation
- **File I/O**: Load and save files
- **Syntax Highlighting**: Basic keyword highlighting

### Editor Modes

- **Normal Mode**: Navigation and commands
- **Insert Mode**: Text insertion
- **Command Mode**: File operations

## Popup System

### Components

- **Message Boxes**: Information, warnings, errors
- **Input Dialogs**: Text input collection
- **Confirmation**: Yes/no prompts
- **Progress Bars**: Operation status display

### GUI Framework Detection

The system includes detectors for GUI frameworks in BexVM programs:

- **Tkinter**: Python GUI library
- **Pygame**: Game development
- **Qt/PyQt**: Cross-platform GUI
- **Dear PyGui**: Immediate mode GUI

## Graphics Integration

### Font Rendering

- **Bitmap Font**: Built-in monospace font
- **Color Support**: 24-bit RGB colors
- **Text Positioning**: Pixel-perfect placement

### Framebuffer Access

- **Direct Drawing**: Pixel and rectangle operations
- **Text Rendering**: String display with colors
- **Screen Management**: Clear, scroll, update operations

## Input Handling

### Keyboard Input

- **Scancode Processing**: Raw keyboard input
- **Key Mapping**: Special keys and modifiers
- **Line Editing**: Cursor movement, history navigation

### Event Loop

```rust
fn run(&mut self, fb: &mut Framebuffer) {
    loop {
        // Process keyboard input
        // Update display
        // Handle commands
    }
}
```

## Multi-drive Support

### Drive Management

- **12 Drives**: Alpha through Mu
- **Mount Status**: Automatic detection
- **Current Directory**: Per-drive working directory
- **Drive Commands**: `drives`, `cd`, `pwd`

### Path Resolution

- **Absolute Paths**: `/drive/path`
- **Relative Paths**: `file.txt`
- **Drive Switching**: `cd /alpha/`, `cd /beta/`

## Error Handling

### Command Errors

- **Invalid Commands**: "Unknown command" messages
- **File Not Found**: Path resolution failures
- **Permission Errors**: Read-only file access
- **Syntax Errors**: Malformed command input

### Recovery

- **Graceful Degradation**: Continue operation on errors
- **Error Messages**: Clear user feedback
- **Logging**: Serial output for debugging

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.