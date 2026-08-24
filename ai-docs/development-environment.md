# Среда разработки

Проверено: 2026-08-24 после обновления toolchain.

## Текущий Windows-хост

- Host target: `x86_64-pc-windows-msvc`.
- Rust установлен через rustup: `rustc 1.98.0`, `cargo 1.98.0`.
- Установлены компоненты `rustfmt`, `clippy`, `rust-src`, `rust-docs`.
- Установлен target `x86_64-pc-windows-msvc`.
- Git: `2.45.1.windows.1`.
- Visual Studio 2022 Community установлен.
- MSVC: `14.44.35207`; `cl.exe` и `link.exe` присутствуют.
- Windows SDK: `10.0.28000.0` и `10.0.19041.0`.
- `cl.exe` не находится в обычном PATH, что допустимо вне Developer Shell.
  Cargo build и локальный Iroh smoke test успешны, следовательно MSVC linker и
  Windows SDK доступны Rust toolchain.
- CMake, Ninja и protoc в PATH не найдены. Для начального Iroh/Rust M0 они не
  считаются обязательными; добавлять только при появлении подтверждённой
  зависимости.

## Совместимость

Актуальный `iroh 1.0.3` требует Rust `1.91`. Текущая ветка OpenMLS также
указывает Rust `1.91`. Установленный Rust `1.98.0` удовлетворяет этим
требованиям. Workspace задаёт MSRV `1.91`, а `rust-toolchain.toml` фиксирует
toolchain `1.98.0` для воспроизводимой разработки.

Выполненная проверка:

1. `rustc --version --verbose` и `cargo --version` — успешно.
2. Компиляция минимального executable — успешно.
3. `cargo fmt --all -- --check` — успешно.
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — успешно.
5. `cargo test --workspace --all-targets` — 2 теста успешно.
6. Два локальных Iroh endpoint обменялись сообщением и acknowledgement — успешно.

Публичный relay пока не проверялся: M0.1 использовал ticket с локальными IP
адресами и direct connection на одном хосте.

## Решения, которые ещё нельзя фиксировать

- Версию OpenMLS следует выбрать по стабильному crates.io-релизу, а не по `main`.
- Protobuf пока не выбран, поэтому отсутствие системного `protoc` не является
  блокером; при выборе Protobuf желательно использовать воспроизводимый
  vendored protoc.
- CMake/Ninja устанавливать заранее не требуется.
