// 导入 `ChainedPics` 结构，这是来自 `pic8259` crate 的一个结构，表示两个级联的 8259 可编程中断控制器（Programmable Interrupt Controller, PIC）
use pic8259::ChainedPics;
// 导入 `spin` crate，它提供自旋锁等同步原语
use spin;

// 定义常量 `PIC_1_OFFSET` 表示第一块 PIC 的中断向量偏移量。`32` 是中断号起始处，主要用于映射可编程中断控制器到 IDT 中的位置
pub const PIC_1_OFFSET: u8 = 32;
// 类似地定义第二块 PIC 的偏移(40)，因为 8259A PIC 最多能处理8个映射所以距离前者增加了8
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// 声明一个名为 `PICS` 的静态变量，并存放在一个 `spin::Mutex` 锁内保障同步访问，初始化代码为安全敏感操作所以标记成了unsafe。使用之前声明的两个偏移值来实例化两块 PIC 控制器并且将其级联起来
pub static PICS: spin::Mutex<ChainedPics> = spin::Mutex::new(
    unsafe {ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)}
);

// 这里通过派生(`derive`)特性给我们的 `InterruptIndex` 枚举添加调试、克隆和复制功能。
#[derive(Debug, Clone, Copy)]
// 同时用 `#[repr(u8)]` 属性确保枚举底层数据类型为 u8
#[repr(u8)]
pub enum InterruptIndex {
    // 定义枚举，其中每一项代表重要硬件中断的索引值。首项 'Timer' 设定等同于之上对齐基准静态常量处 (即中断向量起点数)，而 'Keyboard' 自动递增位次序(33)
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    // 允许你把枚举内项直接转换成相应的u8数字表达
    pub fn as_u8(self) -> u8 {
        self as u8
    }
    // 转usize 方法借助已确定拥有从'u8 转换成更通用usize' 操作可能性前提下 获取值 —— 这种设计是经过考虑方便Rust标准库和诸如数组索引等场景使用需要整型下界但数字大小本质不大时采用小占空间排序）
    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

// 1. ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)
// `ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)` 是 `ChainedPics` 结构体中的一个 `new` 函数，你传入两个参数（PIC控制器的中断向量偏移量）来创建一个新的 `ChainedPics` 实例。这个实例代表了一对级联的 8259 可编程中断控制器，它用于通知x86系统何时和如何处理硬件中断。

// 这个函数调用被标记为unsafe是因为直接与硬件交互相关并且必须对底层系统有足够理解来保证安全性，任何不当操作都可能导致未定义行为或系统崩溃。

// 而 `spin::Mutex::new(...)` 包裹着创建出来的 `ChainedPics` 实例，则提供了一个自旋锁（spin lock）。自旋锁是一种同步机制，在尝试获取锁以访问受保护资源（本案例即 `ChainedPics`）失败时候不会阻塞当前线程而是等待，也就是持续循环检查是否能获得锁（"自旋"）。

// 将 `ChainedPics` 实体包含在一种线程安全结构如 Mutex之内非常重要因为你通常希望在多核或支援抢占式任务情景下对PIC进行正确管理避免出现资源竞争状态产生潜在风险。使用自旋锁适合中断处理或其他低延迟状况需求场合，因其避免了上下文切换造成开销问题所以在此类情形下经常被采用。而且考虑到没有办法从中断上下文里做可休眠(sleeping) 动作所以选它特别恰当(即确保资料结构只存在单访问点但同时没进入无穷空转浪费CPU能量).

// 最后返回值实质是一个包含了初始化好且具有经过包裹控制权限 MCU 控制器实例静态变量期待曰后进行查询配置使用等活动推动 。


// 2. ChainedPics的两个参数分别是什么意思 什么作用?
// `ChainedPics::new` 的两个参数是两块级联 8259 可编程中断控制器（PIC）的偏移量，它们定义了每个 PIC 控制器处理中断的起始向量号。

// 在 x86 系统中，CPU 处理硬件中断会使用一个名为中断向量的数字来标识特定的中断。当使用 8259 PIC 的系统上，这些向量号与 IRQ（interrupt request lines）一一对应，用于确保 CPU 能够区分不同来源的硬件中断请求并正确响应。

// 具体到这两参数：

// 1. `PIC_1_OFFSET`: 这是主 PIC (Primary PIC) 的偏移量。因为 Intel 架构预留了前32个中断向量给 CPU 内部异常使用（如除零错误、页面错误等），所以通常从第 32 号向量开始用作外部硬件中断。设置该值保证了主 PIC 处理的 IRQs 映射到 IDT(Interrupt Descriptor Table) 中不与内部异常冲突的地方；也即实现IRQ0-7映射至 32 到 39 号向量。

// 2. `PIC_2_OFFSET`: 对应从属 PIC (Secondary or Slave PIC) 的偏移量。由于一个单独的PIC只能处理8个IRQs，而大多数系统都有超过8个外设可能产生IRQs, 因而采取二枚电路板级联而得方式增加可监察能力范围——从属版对齐注意点放队列后头变职负责IRQ8-15总数16条线索内容；凭借此项布置可让相关性转接IDT入口介于40至47号顺位。

// 给出这样设计取舍意义在于既满足需求同时避免了和CPU内建异常或指令集预判与未来可能扩展措施冲突局面
