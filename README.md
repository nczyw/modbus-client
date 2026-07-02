# Modbus Client

A cross-platform Modbus client application with a graphical user interface, built with Rust. Supports both Modbus TCP and Modbus RTU (serial) protocols for reading and writing coil and register data.

![Rust](https://img.shields.io/badge/Rust-2024-orange)
![Version](https://img.shields.io/badge/Version-0.1.0-blue)

## Features

- **Dual Protocol Support** — Modbus TCP and Modbus RTU (serial port)
- **Full Data Access** — Read/write Coils, Discrete Inputs, Input Registers, and Holding Registers
- **Rich Data Types** — Supports `U16`, `I16`, `U32`, `I32`, `F32`, `U64`, `I64`, `F64` register interpretations
- **Multiple Display Formats** — Decimal (DEC), Hexadecimal (HEX), Binary (BIN), Octal (OCT)
- **Byte/Word Swap** — Configurable byte swap and word swap for multi-word register values
- **Configurable Polling** — Adjustable polling interval and timeout
- **Flexible Addressing** — Configurable count and offset for each data region
- **Chunked Reading** — Automatically reads in 32-register chunks to comply with Modbus protocol limits
- **Dark/Light Theme** — Toggle between dark and light UI themes
- **UI Scale Factor** — Adjustable display scale via command-line argument
- **Chinese & Emoji Font Support** — Bundled Alibaba PuHui Ti and Noto Emoji fonts
- **Error Handling** — Real-time error display with timeout, transport, and business logic error reporting

## Screenshots

> *(Add screenshots here)*

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024, Rust 1.85+)

### Build

```bash
git clone https://github.com/your-repo/modbus-client.git
cd modbus-client
cargo build --release
```

### Run

```bash
# Default (scale factor 1.0)
cargo run --release

# Custom UI scale factor
cargo run --release -- --scale 1.5

# Short form
cargo run --release -- -s 2.0
```

### Command-Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--scale` | `-s` | `1.0` | UI display scale factor (e.g., 1.0, 1.5, 2.0) |

## Usage

### Connection

1. Select the protocol type (**TCP** or **RTU**) from the dropdown
2. Configure connection parameters:
   - **TCP**: IP address and port (default: `127.0.0.1:502`)
   - **RTU**: Serial port, baudrate, parity (None/Odd/Even), and slave ID
3. Set the polling interval and timeout
4. Click **Connect** / **Disconnect** in the status bar

### Data Table

The main table displays four Modbus data regions side by side:

| Column | Data Region | Function Code | Access |
|--------|-------------|---------------|--------|
| Coils | Coils | FC1 (read) / FC5 (write) | Read/Write |
| Discrete Inputs | Discrete Inputs | FC2 | Read-Only |
| Input Registers | Input Registers | FC4 | Read-Only |
| Holding Registers | Holding Registers | FC3 (read) / FC6/16 (write) | Read/Write |

- Each column header allows adjusting the **count** and **offset** of the data region
- Coils can be toggled directly via checkboxes (writes immediately)
- Holding Registers can be edited by **left-clicking** a value cell
- Register display format and data type can be changed by **right-clicking** a value cell

### Register Settings Dialog

Right-click on any Input Register or Holding Register cell to open the settings dialog:

- **Data Type**: Choose from `U16`, `I16`, `U32`, `I32`, `F32`, `U64`, `I64`, `F64`
- **Display Format**: Choose from `DEC`, `HEX`, `BIN`, `OCT`

### Status Bar

- ● Green/Red indicator showing connection status
- **Connect/Disconnect** button
- **word-swap** and **byte-swap** checkboxes
- Error messages (displayed in red when errors occur)
- Frame time display (ms)
- 🌙/☀️ theme toggle button

## Architecture

The application uses a multi-threaded architecture:

- **UI Thread** — Runs the eframe/egui GUI event loop
- **Modbus Thread** — Runs a dedicated tokio async runtime for Modbus communication

Communication between threads:

- **Command Channel** (`mpsc`) — UI sends commands (Connect, Disconnect, WriteCoil, WriteHoldRegister) to the Modbus client
- **Error Channel** (`watch`) — Modbus client sends error messages to the UI
- **Shared Data** (`Arc<ShareData>` with `ArcSwap`) — Lock-free concurrent data sharing for real-time display

## Tech Stack

| Component | Technology |
|-----------|------------|
| GUI | [eframe](https://github.com/emilk/egui) / [egui](https://github.com/emilk/egui) |
| Tables | [egui_extras](https://github.com/emilk/egui) |
| Async Runtime | [tokio](https://github.com/tokio-rs/tokio) |
| Modbus Protocol | [tokio-modbus](https://github.com/slowtec/tokio-modbus) |
| Serial Port | [tokio-serial](https://github.com/berkowski/tokio-serial) |
| CLI Parsing | [clap](https://github.com/clap-rs/clap) |
| Error Handling | [anyhow](https://github.com/dtolnay/anyhow) / [thiserror](https://github.com/dtolnay/thiserror) |
| Concurrent Data | [arc-swap](https://github.com/KvltKrakr/arc-swap) |

## Project Structure

```
modbus-client/
├── Cargo.toml
├── fonts/
│   ├── AlibabaPuHuiTi-3-55-Regular.ttf   # Chinese font
│   └── NotoEmoji-VariableFont_wght.ttf    # Emoji font
├── src/
│   ├── main.rs                            # Entry point, font loading, UI launch
│   ├── modbus.rs                          # Module declarations
│   ├── modbus/
│   │   ├── modbus_client.rs               # Modbus client, config, connection, polling
│   │   └── share_data.rs                  # Shared data, register types, display formats
│   ├── ui.rs                              # Module declarations
│   └── ui/
│       └── app_ui.rs                      # GUI layout, dialogs, user interaction
```

## License

This project is licensed under the [MIT License with Attribution Requirement](LICENSE).

**Attribution Requirement**: Any use, copy, modification, merge, publication, distribution, sublicense, or sale of this software (including derivative works and binary distributions) must clearly and prominently display the original repository and author information (**WenJun**) in the most conspicuous manner possible — either within the software's user interface (e.g., title bar, about dialog, status bar) or alongside the distributed binary files (e.g., README, LICENSE, splash screen). See the [LICENSE](LICENSE) file for full details.