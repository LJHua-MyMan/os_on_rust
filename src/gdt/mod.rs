// 在 Rust 编写裸金属或操作系统时，对全局描述符表（GDT）和任务状态段（TSS）进行管理常常是必备步骤

// `lazy_static`允许你创建在程序运行时初始化一次且只有一次的静态变量。
use lazy_static::lazy_static;
// 从`x86_64` crate（Rust里包和库的术语）中导入了名为`Segment`的trait，该trait定义了与x86特定CPU段相关的功能
use x86_64::instructions::segmentation::Segment;
// 这行代码从同一个crate中导入了名为`SegmentSelector`的结构体，它代表了在GDT（全局描述符表）或LDT（局部描述符表）中选择器索引。
use x86_64::registers::segmentation::{SegmentSelector};
// 这行代码从库导入两个类型：`Descriptor`, 它是段描述符的表示；以及 `GlobalDescriptorTable`, 是GDT本身的抽象表示
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
// 这里导出了名为 `TaskStateSegment`(TSS) 的结构体, TSS用于现代x86 CPU实现任务切换等高级操作。
use x86_64::structures::tss::TaskStateSegment;
// 这行代码导出 `VirtAddr`, 一个类型别名用于表示虚拟地址，即内存地址转换后在CPU访问权限范围内而不是物理内存位置
use x86_64::VirtAddr;

// 声明并初始化一个公共常量(`pub const`)叫做 `DOUBLE_FAULT_IST_INDEX`, 类型为无符号16位数(`u16`)，值初始化为0
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// 定义了一个 Rust 结构体（struct）命名为 "Selectors"。该结构体有两个字段：第一个字段 code_selector 表示代码段选择子；第二个 tss_selector 是TSS(Task State Segment) 的段选择子。每个字段都使用前面提到过的结构体 SegmentSelector。
struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

// 这部分代码使用`lazy_static!`宏来定义两个静态的引用：`TSS`和`GDT`。这些是在操作系统或裸机上下文中使用x86_64架构时需要的低级结构
lazy_static! {
    // 此行定义一个名为`TSS`的静态可变引用，类型为 `TaskStateSegment`，会在第一次访问时进行初始化，并且保持其状态直至程序结束
    static ref TSS: TaskStateSegment = {
        // 创建一个新的 `TaskStateSegment` 结构体实例，命名为`tss`
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            // 为这个特定的中断堆栈预留多少空间（4096字节x5）
            const STACK_SIZE: usize = 4096 * 5;
            // 定义了一个静态(全局)、可变(mutable)数组 `STACK`, 占用 `STACK_SIZE` 大小人字节, 初始值全为0.
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            // 获取刚才定义的堆栈区域起始指针(`stack_start`) 的虚拟地址
            // 使用 `unsafe {}`, 因为对全局可变状态进行操作谨防数据竞争条件，在 Rust 中通常被视作不安全行为
            let stack_start = VirtAddr::from_ptr(unsafe {&STACK});
            // 算出应该使用区域终点(`stack_end`) 的虚拟地址。
            let stack_end = stack_start + STACK_SIZE;
            // 因为 CPU 总是从所指定地址向下增长堆栈，在任务或中断发生时往下放置内容，所以我们提供空间终点作为开始位置
            // 把计算出来的 `stack_end` 赋给了 TSS 的 `interrupt_stack_table` 中第 `DOUBLE_FAULT_IST_INDEX` 项。换句话说，指定如果CPU遇到双重故障(double fault)中断时，应该使用位于 `stack_end` 开始向下增长的栈
            stack_end
        };
        tss
    };
    // 使用 `lazy_static!` 定义一个全局、静态生命周期的变量 `GDT`，该变量只会被初始化一次，并且其类型是一个元组 `(GlobalDescriptorTable, Selectors)`。`GDT` 代表全局描述符表，而 `Selectors` 是我们将要定义的自定义结构体，它包含两个段选择器
    static ref GDT:(GlobalDescriptorTable, Selectors) = {
        // 创建了一个新的空的 `GlobalDescriptorTable` 结构实例，并命名为 `gdt`。由于接下来需要向 `gdt` 中添加条目，因此它被声明为可变（mut）
        let mut gdt = GlobalDescriptorTable::new();
        // 在全局描述符表中添加一个内核代码段并返回该段的选择器。该代码段的具体设置（如基址和界限）通常由操作系统决定；在这种情况下，采用了默认内核代码段配置
        // 当执行 `Descriptor::kernel_code_segment()` 方法时，该方法配置并返回代表代码段属性（如基址、界限和访问/执行权限等）信息汇总结构体实例；随后使用 `gdt.add_entry(...)` 将此信息条注册至GDT 并返回相关新条目标识 “选择子”。这个选择子可以加载到CPU的代码段寄存器(CS)，使得它能够用正确权限去正确位置取得将要运行指令集完整概貌.
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        // 添加了一个任务状态段(`TaskStateSegment`)到GDT，并返回对应的选择器。传递给此方法的参数是对前面定义好且通过 `lazy_static!` 初始化好的静态引用变量 `TSS` 的引用
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors{code_selector, tss_selector})
    };
}

// 用来初始化我们之前定义的全局描述符表（GDT）
pub fn init() {
    // 通过 `use` 关键字将 `CS` 导入当前作用域，它是代码段寄存器（Code Segment Register）的简写，在x86架构中用来存储当前正在执行指令的内存段的选择器
    use x86_64::instructions::segmentation::CS;
    // 导入 `load_tss` 函数到当前作用域。该函数用于加载任务状态段寄存器（task state segment register, TR）
    use x86_64::instructions::tables::load_tss;
    // 调用 `load` 方法来加载我们之前定义和初始化好的全局描述符表（GDT）。这会将GDT注册到CPU内部以便后续访问和使用。记住，GDT是个元组 `(GlobalDescriptorTable, Selectors)`，所以 `.0` 是访问第一个元素，即实际的全局描述符表实例
    GDT.0.load();
    // 由于直接操作硬件层面上的段寄存器存在可能危险行为或特定要求下才允许操作属性，所以相应功能包裹在 `unsafe {}` 块中
    unsafe {
        // 使用之前保存于 Selectors 中的 code_selector 来设置 CS 寄存器。这会更新正在运行代码线程所参考代码段选择子为我们预设好欲指向与保护模式有关部分
        CS::set_reg(GDT.1.code_selector);
        // 调用库提供 `load_tss` 方法，并传递 tss_selector 也就是任务状态段对应选择子。此动作告知CPU对新TSS实例其管理信息位置执行更新
        load_tss(GDT.1.tss_selector);
    }
}

// `CS::set_reg(GDT.1.code_selector);` 这行代码本身并不直接实现从保护模式到长模式的转换，也就是说它不切换CPU运作状态。
// 在 x86_64 架构中，进入长模式（Long Mode）是一个几步进行的复杂过程。具体来说，需要：
// 1. 开启分页（Paging），将CR0寄存器的分页位置1。
// 2. 加载一个支持64位模式（即兼容长模式）的GDT。
// 3. 将IA32_EFER MSR寄存器的LME位（长模式启用位）置为1。
// 4. 设置CR4寄存器以启用物理地址扩展(PAE)。
// 5. 更新CR3寄存器以指向适当的页表基址。
// 6. 设置CR0寄存器以启用保护模式且关闭实模式。

// 最后一步开启了CPU中断控制之前кодыты，并使得内核跳转至符合长模态规定下形如可“解析”64-bit 码偏移量等理解所需特性相关新代码段执行那里 —— 这个时候 `CS` 寄存器会被更新为一个新值来反映变化情况。

// 因此，在这整个序列动作中你提及那行 `CS::set_reg(GDT.1.code_selector);` 用处在于：

// - 在进行前述若干设置后，
// - 排定 GDT 中目标段条目可参照街
// - 可能立即或稍后根据预案更新 CS 寄存器，

// 意图引导 CPU "认知"及遵循 设计上默认推荐引导代码段描述比 。但要注意切换到长模态绝非单靠此行完成 ，仅为必要配套动作之部分
