
#![no_std] // 不链接Rust标准库
#![no_main] // 禁用所有Rust层级的入口点
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
#[warn(unused_imports)]
use cjn_os::println;
use cjn_os::vga_buffer;

// 将会在panic时调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("{}", _info);
    cjn_os::hlt_loop();
}


#[no_mangle] //不重整函数名
// 定义一个符合C调用规范的公开函数 `_start`。由于使用 `-> !` 表明这个函数永不返回.
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();
    // 进入无限循环防止 `_start` 函数,返回也确保内核不会意外退出到未定义行为状态中去
    cjn_os::hlt_loop();
}
