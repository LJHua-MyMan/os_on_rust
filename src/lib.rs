#![no_std]
// 表示程序不使用常规的入口点命名（例如 `main` 函数），这是因为大多数操作系统都有自己特定的入口点要求
#![no_main]
// 启用一个尚未稳定的 Rust 功能，允许定义使用 `"x86-interrupt"` 调用约定的函数。这对于设置处理x86中断所需的正确函数签名至关重要
#![feature(abi_x86_interrupt)]

// 告知编译器应有相应模块存在，并指示它去特定位置寻找这些模块定义
// - `interrupts`: 处理CPU中断和异常。
// - `vga_buffer`: 控制文本模式VGA显示缓冲区输出。
// - `gdt`: 设置全局描述符表(Global Descriptor Table)，它定义了不同内存段(segment)的权限和属性。
// 这表示正在声明（declare）三个模块：`interrupts`、`vga_buffer` 和 `gdt`。通过使用 `mod` 关键字，告诉 Rust 编译器期望在当前 crate 的文件系统中找到与模块同名的文件或目录。
// - 如果是文件，则模块的内容将会来自于一个同名的 `.rs` 文件。例如，对于 `mod interrupts;`，编译器会查找一个叫做 `interrupts.rs` 的文件。
// - 如果是目录，则模块的内容将会来自于该目录下的 `mod.rs` 文件。例如，对于 `mod gdt;` 如果有一个名为 `gdt/` 的目录存在，那么编译器会查找 `gdt/mod.rs
pub mod interrupts;
pub mod vga_buffer;
pub mod gdt;

pub fn init() {
    // 加载GDT
    // 初始化全局描述符表(GDT)。GDT是保护模式下x86 CPU使用来区分不同内存区域特性（如基址、大小和访问权限等）的数据结构
    gdt::init();

    // 加载中断和异常处理
    // 初始化IDT（中断描述符表），此数据结构用来告诉CPU各种异常和中断应该由哪些处理函数来处理
    interrupts::init_idt();
    // 初始化可编程中断控制器(PIC)，配置它以接收硬件中断。因为PIC相关操作可能会引起未定义行为，所以需要放在unsafe块内执行。
    unsafe {interrupts::pics::PICS.lock().initialize()};
    // 开启CPU中断，使得CPU能够响应外部设备发起的IRQ和其他形式的硬件请求
    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> !{
    loop {
        // 这个无限循环被设计成一个安全停止执行流程，并等待下一个可用中断事件。每次循环调用汇编指令HLT (Halt)，暂停CPU执行直到发生下一次硬件中断。返回类型 `!` 表示该函数永远不会返回
        x86_64::instructions::hlt;
    }
}



// 1. #![no_std]是工程里每个rs都要使用吗？为什么有的rs没使用？有的rs比如这个lib.rs又使用
// 属性 `#![no_std]` 通常只在 crate 根（如库的根文件 lib.rs 或二进制项目的 main.rs）中设置一次。这是因为 `#![no_std]` 是一个属性(attribute)，它应用于整个 crate 的配置，而不仅仅是单个模块。
// 当你在 crate 的根文件中声明 `#![no_std]`，你告诉编译器当前这个 crate 不链接到 Rust 的标准库（std），而是使用核心库（core），后者是适用于裸机或嵌入式系统的功能子集，没有操作系统特性依赖。
// 如果看到某些 `.rs` 文件中没有使用 `#![no_std]`，那大概有以下几种情况：
// 1. **它们不是根文件**：只需要在根文件中声明一次。
// 2. **条件编译**：有时候某些代码片段可能会基于特定条件编译。比如，在支持标准库时无需 `#![no_std]`。
// 3. **错误或不一致**：如果确实需要保持整个crate都不依赖标准库，并且某个文件遗漏了这个属性，那么可能是一个错误。但实际上只要根文件声明了就足够了。

// 所以，在多数情况下，你将会在每个独立编译单元的顶部看到 `#![no_std]` 声明一次即可；对于一个库来说通常是位于 `lib.rs`, 对于二进制项目则通常位于 `main.rs`. 这样做可以确保整个crate都符合无标准库的运行环境需求。

// 我理解这里lib.rs和main.rs同层级，所以也加了#![no_std].
