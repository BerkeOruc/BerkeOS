# BerkeBex - BerkeOS Python → .bex Compiler

**Berkebex**, BerkeOS için Python programlarını `.bex` bytecode'a derleyen cross-compiler'dır.

## Quick Start

### 1. Build Berkebex (Host Computer)

```bash
cd berkebex
cargo build --release
```

### 2. Compile Python to .bex

```bash
# Basit Python script
echo 'print("Hello from Python!")' > hello.py
./target/release/berkebex --python hello.py -o hello.bex

# Fibonacci örneği
./target/release/berkebex --python fib_test.py -o fib_test.bex
```

### 3. Run in BerkeOS (QEMU)

```bash
# BerkeOS ISO build et
cd ..
chmod +x build.sh && ./build.sh

# QEMU'da çalıştır
chmod +x run.sh && ./run.sh
```

BerkeOS açıldığında berkesh shell'de:

```sh
# .bex dosyasını çalıştır
berun hello.bex

# VEYA direkt Python çalıştır (berun --python)
berun --python hello.py
```

---

## Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  HOST COMPUTER (Linux/Mac/Windows)                         │
│                                                             │
│  1. Python kodu yaz                                         │
│     → hello.py                                              │
│                                                             │
│  2. Berkebex ile derle                                       │
│     → berkebex --python hello.py -o hello.bex               │
│                                                             │
│  3. .bex dosyasını al                                       │
│     → hello.bex (273 bytes bytecode)                        │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      │ .bex dosyasını BerkeFS'ye kopyala
                      │ (QEMU'da BerkeOS boot edince disk'e yaz)
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  BERKEOS (QEMU / Real Hardware)                            │
│                                                             │
│  berkesh> berun hello.bex                                  │
│  → "Hello from Python!"                                     │
│                                                             │
│  VEYA direkt Python çalıştır:                              │
│  berkesh> berun --python hello.py                          │
│  → Otomatik derle + çalıştır                                │
└─────────────────────────────────────────────────────────────┘
```

---

## Features

- **Pure Python Syntax** — Python 3.x subset'i, gerçek Python kodu yaz
- **Functions** — `def`, `return`, recursive fonksiyonlar
- **Classes** — `class`, `__init__`, inheritance, methods
- **Exception Handling** — `try`, `except`, `raise`
- **BerkeOS API** — `berkeos.process`, `berkeos.file`, `berkeos.display`
- **Stdlib** — `json`, `re` modülleri
- **GUI Detection** — tkinter/PyQt kullanımında uyarı

---

## Usage

### Berkebex CLI (Host Computer)

```bash
# Python → .bex derle
berkebex --python script.py -o output.bex

# .bex dosya bilgisi
berkebex info program.bex

# Help
berkebex --help
```

### BerkeOS Shell Commands

```sh
# .bex çalıştır
berun program.bex

# Python dosyasını derle + çalıştır
berun --python script.py

# Berkepython shell (berkebex ile)
berkepython script.py
```

> **Not:** `berun` ve `berkepython` komutları BerkeOS shell'inde çalışır, host'ta değil.

---

## Supported Python Syntax

### Variables & Arithmetic
```python
x = 10
name = "BerkeOS"
is_active = True

result = (x + 5) * 2 - 10 / 2
power = 2 ** 10
mod = 17 % 5
```

### Functions
```python
def add(a, b):
    return a + b

def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

print(fibonacci(10))  # → 55
```

### Classes & OOP
```python
class Animal:
    def __init__(self, name):
        self.name = name
    
    def speak(self):
        return "..."

class Dog(Animal):
    def __init__(self, name, breed):
        super().__init__(name)
        self.breed = breed
    
    def speak(self):
        return "Woof!"

dog = Dog("Buddy", "Golden Retriever")
print(dog.name)
print(dog.speak())
```

### Exception Handling
```python
try:
    x = 10 / 0
except:
    print("Division by zero!")

try:
    result = int("not a number")
except ValueError:
    print("Invalid number")
```

---

## BerkeOS API

BerkeOS sistem çağrılarına `berkeos` modülünden eriş:

```python
import berkeos

# Process
berkeos.process.sleep(1000)      # 1 saniye bekle
pid = berkeos.process.getpid()    # Process ID al

# File operations
handle = berkeos.file.open("/data.txt", "r")
content = berkeos.file.read(handle, 256)
berkeos.file.write(handle, "hello")
berkeos.file.close(handle)

# Display (Framebuffer)
berkeos.display.clear(0x000000)                 # Ekranı temizle
berkeos.display.draw_pixel(100, 100, 0xFF0000)  # Kırmızı piksel
berkeos.display.draw_rect(50, 50, 200, 100, 0x00FF00)
berkeos.display.draw_text(10, 10, "Hello!", 0xFFFFFF)

# Input
key = berkeos.input.key()  # Klavye input bekle

# Window
win = berkeos.window.new("My App", 640, 480)
```

### API Reference

| Module | Function | Açıklama |
|:-------|:---------|:---------|
| **process** | `sleep(ms)` | ms milisaniye bekle |
| **process** | `getpid()` | Process ID döndür |
| **file** | `open(path, mode)` | Dosya aç ("r", "w", "a") |
| **file** | `read(handle, size)` | Dosyadan oku |
| **file** | `write(handle, data)` | Dosyaya yaz |
| **file** | `close(handle)` | Dosyayı kapat |
| **display** | `clear(color)` | Ekranı temizle (0xRRGGBB) |
| **display** | `draw_pixel(x, y, color)` | Piksel çiz |
| **display** | `draw_rect(x, y, w, h, color)` | Dikdörtgen çiz |
| **display** | `draw_text(x, y, text, color)` | Metin yaz |
| **input** | `key()` | Klavye input bekle |
| **window** | `new(title, w, h)` | Pencere oluştur |

### Colors

24-bit RGB hexadecimal format: `0xRRGGBB`

| Renk | Hex |
|:-----|:----|
| Kırmızı | `0xFF0000` |
| Yeşil | `0x00FF00` |
| Mavi | `0x0000FF` |
| Beyaz | `0xFFFFFF` |
| Siyah | `0x000000` |
| Sarı | `0xFFFF00` |

---

## Stdlib Modules

### json
```python
import json

data = {"name": "BerkeOS", "version": 1}
text = json.dumps(data)           # → '{"name": "BerkeOS", "version": 1}'
parsed = json.loads('{"x": 10}')  # → {"x": 10}
```

### re (Regular Expressions)
```python
import re

match = re.match("[0-9]+", "123abc")
if match:
    print("Found:", match)
```

---

## GUI Detection

Berkepython GUI framework'lerini tespit eder ve uyarı verir:

```bash
$ berkebex --python gui_app.py
Warning: GUI framework 'tkinter' detected. BerkeOS does not support graphical UI libraries.
gui_app.py -> gui_app.bex
```

Desteklenen GUI framework'leri:
- `tkinter`, `Tkinter`
- `PyQt5`, `PyQt6`, `PySide2`, `PySide6`
- `pygame`
- `wx`, `wxPython`
- `matplotlib` (figure kullanımı)
- `kivy`, `arcade`, `turtle`

---

## Error Messages

| Hata | Sebep | Çözüm |
|:-----|:------|:------|
| `SyntaxError: unexpected token` | Geçersiz syntax | Parantez/ayraç kontrol et |
| `NameError: 'xyz' is not defined` | Tanımlanmamış değişken | Değişkeni tanımla |
| `TypeError: unsupported operand` | Yanlış tipler | Sayı-sayı, string-string işlem yap |
| `ZeroDivisionError` | Sıfıra bölme | Bölmeden önce kontrol et |
| `FileNotFoundError` | Dosya yok | Dosya yolunu kontrol et |

---

## Limitations

- **No pip install** — Harici paketler desteklenmez
- **No native extensions** — .pyd, .so dosyaları çalışmaz
- **No full stdlib** — Sadece `json`, `re` modülleri
- **No threading** — Çoklu iş parçacığı yok
- **No network sockets** — Ağ API'leri henüz yok
- **Integers only** — Floating-point minimal destek
- **Framebuffer required** — Grafik modu gerekli (VGA text mode'da çalışmaz)

---

## .bex File Format

```
┌──────────────────────────────────────┐
│ Magic: 0x42455831    ("BEX1")       │
│ Version: 1                          │
├──────────────────────────────────────┤
│ Name Length + Name                   │
├──────────────────────────────────────┤
│ Constants Pool (i64, strings)        │
├──────────────────────────────────────┤
│ Functions Pool (bytecode ops)        │
└──────────────────────────────────────┘
```

Magic header: `1XEB` (0x42455831)

---

## Build System

```
berkebex/                    # Python → .bex compiler (host'ta çalışır)
├── src/
│   ├── main.rs              # CLI, --python flag
│   ├── parser/              # Python parser (rustpython)
│   ├── ir/                  # AST → IR transformation
│   ├── codegen/             # IR → BexVM bytecode
│   ├── import.rs           # Module import system
│   ├── builtins/            # BerkeOS API, json, re
│   ├── gui_detector/        # GUI framework detection
│   └── stdlib/              # Standard library modules
└── tests/python/            # 52 Python test dosyası

berkeos/                     # Operating System Kernel
├── src/
│   ├── bexvm.rs             # BexVM runtime (berun komutu)
│   ├── shell.rs             # berkesh shell + berkepython
│   └── popup.rs             # GUI warning popup
└── build.sh                 # ISO build script
```

---

## Roadmap

- [x] Python parser (rustpython)
- [x] IR → BexVM codegen
- [x] Functions, classes, methods
- [x] try/except, with statement
- [x] Import system
- [x] json, re stdlib
- [x] berkeos.* API
- [x] GUI detection
- [x] berkepython shell command
- [x] Popup warning system
- [ ] BerkeOS içinde editor ile yazıp derleme
- [ ] Code editor içinde .bex preview
- [ ] Floating-point support
- [ ] Network API

---

## License

Apache 2.0 — Copyright 2024-2026 Berke Oruç
