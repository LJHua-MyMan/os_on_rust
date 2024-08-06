use lazy_static::lazy_static;
// 从`pc_keyboard` crate（包）导入 `Keyboard` 结构和 `layouts` 模块。该crate提供了处理PC样式键盘输入的方法和数据结构
use pc_keyboard::{Keyboard, layouts};
// 从spin库导入其版本的互斥锁（Mutex）。这种类型的锁特别适合操作系统级应用，因为操作系统不总是可以休眠线程以等待锁释放
use spin::lock_api::Mutex;
// 导入用于低级别I/O端口操作的 `Port` 结构体，与硬件设备进行通信时常用到
use x86_64::instructions::port::Port;
// 从x86_64标准库中导入关于中断描述符表(Interrupt Descriptor Table, IDT)和中断栈帧(Interrupt Stack Frame) 的结构体定义。IDT用于定义中断服务例程(ISRs)，而中断栈帧保存发生中断时CPU寄存器状态
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// 引入前面定义好的枚举 `InterruptIndex` ，代表各个片段(PICS)相关联映射向量编号概念理解工具项
use pics::InterruptIndex;

// 导出当前crate提供的打印函数 "`print!`" 和 "`println!"` 宏，方便其他模块输出信息至控制台或屏幕
use crate::{print, println};

pub mod pics;

lazy_static! {
    // 定义了一个名为 `IDT` 的静态变量
    static ref IDT: InterruptDescriptorTable = {
        // 使用默认构造函数创建一个新的空白IDT实例
        let mut idt = InterruptDescriptorTable::new();
        // 设置debugger breakpoint (调试器断点异常) 中断处理函数
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        // 设置double fault (双重错误）异常 对应中断处理功能
        idt.double_fault.set_handler_fn(double_fault_handler);
        // 将计时器和键盘中断索引映射到相应处理程序
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(time_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt
    };
}

// 加载中断描述符表(IDT)到 CPU。`IDT.load()` 调用实际执行此操作。
pub fn init_idt() {
    IDT.load();
}

// 调试异常处理函数
// `breakpoint_handler` 是断点异常的处理函数，使用 `"x86-interrupt"` 调用约定。当发生断点异常时，此函数会被调用。
// - `_stack_frame`: 包含了发生中断时CPU寄存器状态的 `InterruptStackFrame` 结构体。
// - 函数内部打印一条消息和栈帧信息
extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", _stack_frame);
}

// 双重异常处理函数
// `double_fault_handler` 是双重错误异常的处理函数。
// - `_error_code`: 双重故障给出的错误码（在本例中未使用）。
// - 函数内部打印一条消息和栈帧信息后进入无限循环，因为双重错误通常是致命的，不可能恢复执行；返回类型 `!` 表明该函数不返回
extern "x86-interrupt" fn double_fault_handler(_stack_frame: InterruptStackFrame, _error_code: u64) -> ! {
    println!("EXCEPTION: DOUBLE FAULT\n{:#?}", _stack_frame);
    loop {}
}

// 定时器中断处理函数
// - 每次定时器触发时打印出一个点(`.`)来表示时间流逝。
// - `unsafe {}` 块包含潜在危险操作：锁定 PIC 控制器并发送 EOI (End Of Interrupt)，告知我们已经完成对当前中断的处理；需要unsafe因为如果错误地发送EOI可能导致中断管理混乱
extern "x86-interrupt" fn time_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");

    unsafe {
        pics::PICS.lock().notify_end_of_interrupt(pics::InterruptIndex::Timer.as_u8());
    }
}

// 键盘中断处理函数
// 使用 `"x86-interrupt"` 调用约定，声明一个键盘中断处理器函数。它接收一个 `InterruptStackFrame` 参数 `_stack_frame`，包含发生中断时的CPU寄存器状态（在此函数不直接使用）
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // 在函数内部导入 `pc_keyboard` crate 的相关模块和类型，用于解码键盘扫描码
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    // 使用 `lazy_static!` 定义了一个静态的 `KEYBOARD` 变量，它是一个互斥锁（Mutex），保护 `Keyboard` 结构体实例。这个结构体支持美国104键布局和扫描集1，并且选择忽略控制字符（例如Ctrl组合按键
    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1,
                HandleControl::Ignore)
            );
    }
    // 通过锁获取对 `KEYBOARD` 的访问权限，并将其赋值给变量 `keyboard` 供后续操作使用
    let mut keyboard = KEYBOARD.lock();
    // 创建新的I/O端口对象以读取端口号为0x60的数据，0x60是标准PS/2键盘的数据端口号
    let mut port = Port::new(0x60);
    // 从数据端口读取一个字节大小的扫描码。因为I/O端口读写可能与硬件直接交互且无法保证总是安全有效，所以这里需要使用unsafe块
    let scancode: u8 = unsafe { port.read() };
    // 将扫描码添加到之前初始化的 `keyboard` 实例中并尝试解析出具体的按键事件。如果成功处理按键事件，则输出相应字符或按键信息。
    // - 如果成功解析成Unicode字符，则直接打印该字符。
    // - 如果是特殊按键，则打印其原始按键值的Debug表示形式。
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}",character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }
    // 通过向PIC发送EOI（结束中断信号），通知硬件我们已经完成对当前这个中断处理程序的工作。同样地，因为涉及到底层硬件交互操作必须在unsafe块内执行
    unsafe {
        pics::PICS.lock().notify_end_of_interrupt(pics::InterruptIndex::Keyboard.as_u8());
    }
}

// 1. 为什么double_fault_handler和breakpoint_handler不用发送EOI?
// `double_fault_handler` 和 `breakpoint_handler` 不需要发送结束中断（EOI）信号的原因在于它们处理的是处理器自己生成的异常，而不是外部硬件中断。

// 在 x86 架构中，有两种类型的中断：

// 1. **异常**：由 CPU 内部检测到错误或特殊条件时产生。这些包括除0错误、页面错误、无效操作码等。它们通常与当前执行的代码直接相关，并且可以同步发生。
// 2. **IRQs（中断请求）**：由外部硬件设备产生以通知CPU有事件需要处理，例如键盘输入、计时器触发等。IRQs 需要通过编程中断控制器（如 PIC 或 APIC）来管理。

// 当 CPU 接收到一个异常时，它会立即跳转到相应的异常处理程序来响应该异常；这个过程不涉及外部硬件，并且不需要发送EOI。

// 另一方面，当 CPU 接收一个 IRQ 时，在 IRQ 被服务之后必须向 PIC 发送一个 EOI 信号来告诉它该中断已被处理。如果不这样做，PIC 将会阻止该线（或其他可能更低优先级线）上进一步的中断，因为它认为当前的还没有得到处理。

// 综上所述，在 `double_fault_handler` 和 `breakpoint_handler` 这类针对 CPU 异常的处理函数内发送EOI 是无意义的，因此在实现时不包含此操作。
