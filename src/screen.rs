#[derive(Debug)]
enum Mode {
    ZERO,
    ONE,
    TWO
}

#[derive(Debug)]
struct Screen {
    mode: Mode,
    screen_mem: [u8]
}

