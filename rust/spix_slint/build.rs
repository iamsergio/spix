// SPDX-FileCopyrightText: 2026 Sergio Martins
// SPDX-License-Identifier: MIT

fn main() {
    // The element search API (`ElementHandle`) needs the element names that debug info embeds,
    // and this crate's tests find their window through the `slint!` macro, which reads the
    // variable when it compiles. A UI compiled with `slint-build` gets it from
    // `CompilerConfiguration::with_debug_info(true)` instead.
    println!("cargo:rustc-env=SLINT_EMIT_DEBUG_INFO=1");
}
