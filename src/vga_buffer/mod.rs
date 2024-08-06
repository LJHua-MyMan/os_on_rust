// 引入Rust的格式化模块，用于输出显示
use core::fmt;
// 引入写接口，使得可以使用write!宏来打印
use lazy_static::lazy_static;
use spin::Mutex;
// 引入`Volatile`类型封装内存，确保每次修改都是直接对硬件的
use volatile::Volatile;
use x86_64::instructions::interrupts;


// VGA标准颜色
// 允许未使用代码不被警告
#[allow(dead_code)]
// 为枚举派生Debug、Clone、Copy等trait，方便调试和值复制
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 表示每个枚举值将以u8（一个字节）形式存储
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 表示在内存中该结构体会像其单一字段那样布局，有助于避免布局问题和提高性能
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, bcakground: Color) -> ColorCode {
        // 创建一个新的ColorCode实例。前景色放在低4位，背景色放在高4位，并转换为u8类型进行按位运算后返回
        ColorCode((bcakground as u8) << 4 | (foreground as u8))
    }
}

// 提交到内存中的VGA字符
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 设置此结构体在内存中的表示应遵循C语言的排列方式
// 确保其具有与C语言相同的内存布局；这通常意味着字段会按照它们声明时候顺序紧密排列。
#[repr(C)]
struct ScreenChar {
    // 存储单个字符使用的ASCII码（1个字节)
    ascii_character: u8,
    // 存储包含前景色和背景色信息（合起来也是1个字节）的ColorCode结构体实例
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;
// 定义Tab键对应空格数
const TAB_SIZE: usize = 4;

// 表示 VGA 文本模式下屏幕的整个字符缓冲区
#[repr(transparent)]
struct Buffer {
    // 使用二维数组代表屏幕每个位置的字符信息，并包裹在Volatile内以防止编译器优化掉直接写入操作
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

// 输出器
pub struct Writer {
    row_position: usize,
    column_position: usize,
    color_code: ColorCode,
    // 静态生命周期引用当前VGA缓冲区 允许整个程序运行期间可变地访问这个Buffer
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            0x08 => self.backspace(),
            b'\t' => self.horizontal_tab(),
            b'\n' => self.new_line(),
            b'\r' => self.carriage_return(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line()
                }
                let row = self.row_position.clone();
                let col = self.column_position.clone();
                let color_code = self.color_code.clone();
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });

                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' | b'\r' | b'\t' | 0x08 => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row.clone()][col].write(blank);
        }
    }

    fn new_line(&mut self) {
        self.row_position += 1;
        self.column_position = 0;

        if self.row_position >= BUFFER_HEIGHT {
            // 向上滚屏
            for row in 0..BUFFER_HEIGHT - 1 {
                for col in 0..BUFFER_WIDTH {
                    self.buffer.chars[row.clone()][col.clone()].write(self.buffer.chars[row.clone() + 1][col.clone()].read());
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
        }
    }

    fn backspace(&mut self) {
        if self.column_position > 0 {
            self.column_position -= 1;
        }
    }

    fn carriage_return(&mut self) {
        self.column_position = 0;
    }

    fn horizontal_tab(&mut self) {
        self.column_position += TAB_SIZE - (self.column_position.clone() % TAB_SIZE);
        if self.column_position >= BUFFER_WIDTH {
            self.new_line();
        }
    }

}

// 使用 `lazy_static` 宏定义一个全局静态变量 `WRITER`, 包含了多线程安全互斥锁 (Mutex)。内部保存了一个 `Writer` 结构体实例，用于向VGA缓冲区写入文本。
// - VGA缓冲区的物理地址为 `0xb8000`，通过不安全（unsafe）转换成可变指针以便读写。
// - 设置开始时光标位置和颜色代码。
// - 因为访问裸指针和硬件资源是不安全的操作，所以需要unsafe块
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        row_position: 0,
        column_position: 0,
        color_code: ColorCode::new(Color::LightCyan, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

impl fmt::Write for Writer {
    // 函数 `write_str` 返回一个 `Result` 类型，它是 Rust 中一种标准的返回类型用于包含可能存在的错误信息。`Result` 常常用来表示一个操作可能失败的情况，
    // 在这里它具体为 `Result<(), core::fmt::Error>`。
    // - 我们声明函数`write_str`会返回一个特定统称叫做“结果”的东西 (`Result`)，这过程中只拿到它的其中之一（要么正常结束、要么报错）
    // - `Ok(())`: 表示函数成功执行而没有出错。在这个上下文中，`()`, 也就是空元组，用作 `Ok` 的值部分，相当于表示“没有有效值”，只是简单地表明函数已经成功完成了其任务。
    // - `Err(core::fmt::Error)`: 如果有错误出现，会使用这种形式返回。
    // 这里只有一个有效的返回值, 是因为结果类型 (`Result`) 已经包括了两种可能性：要么成功 (带着成功类型 `()`) 要么失败 (带着错误类型 `core::fmt::Error`)
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        self.write_string(s);
        Ok(())
    }
}

// 定义函数 `_print` 来向VGA缓冲区输出格式化文本。使用 `core::fmt::Write` trait 的 `write_fmt` 方法。
// - 使用了隐藏属性防止其出现在生成的文档中。
// - 调用自定义的 `interrupts::without_interrupts` 函数来确保打印过程中不会被中断，避免死锁等并发问题。
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;

    // 防止死锁
    interrupts::without_interrupts(||{
        WRITER.lock().write_fmt(args).unwrap();
    })
}

// 定义了一个宏 `print!`, 当调用此宏时将展开成对上面定义的 `_print()` 函数的调用，传递给定参数作为格式化参数列表。这个宏可以在crate中任何地方使用
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

// 同样导出了另一个宏 `println!`, 它基于前面的 `print!` 宏但还附加一个换行符 `\n`。第一种形式只输出换行符，第二种形式则输出格式化后内容并追加换行符。
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn print_something() {
    println!("Os start now.\n\n");
    println!("\t----Hello World From cjn's Operating System\n");
    println!("\t\t\t\t\t\t\t\t2024.08.02\n");
}
