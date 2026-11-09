// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The interpreter keeps debug info only when asked for it.
//! This test has its own binary because it sets environment variables.

use slint_interpreter::Compiler;

fn finds_element_by_id(compiler: &Compiler) -> bool {
    let result = spin_on::spin_on(compiler.build_from_source(
        "export component TestCase inherits Window { foo := Rectangle {} }".into(),
        Default::default(),
    ));
    assert!(!result.has_errors(), "{:?}", result.diagnostics().collect::<Vec<_>>());
    let instance = result.component("TestCase").unwrap().create().unwrap();
    i_slint_backend_testing::ElementHandle::find_by_element_id(&instance, "TestCase::foo").count()
        == 1
}

#[test]
fn debug_info_only_when_requested() {
    i_slint_backend_testing::init_no_event_loop();
    // SAFETY: this binary has no other test, so no other thread reads the environment.
    unsafe {
        std::env::remove_var("SLINT_EMIT_DEBUG_INFO");
        std::env::remove_var("SLINT_TEST_SERVER");
        std::env::remove_var("SLINT_MCP_PORT");
    }
    assert!(!finds_element_by_id(&Compiler::default()));

    let mut compiler = Compiler::default();
    compiler.set_keep_debug_info(true, i_slint_core::InternalToken);
    assert!(finds_element_by_id(&compiler));

    unsafe { std::env::set_var("SLINT_EMIT_DEBUG_INFO", "1") };
    assert!(finds_element_by_id(&Compiler::default()));
    unsafe { std::env::remove_var("SLINT_EMIT_DEBUG_INFO") };

    #[cfg(feature = "mcp")]
    {
        unsafe { std::env::set_var("SLINT_MCP_PORT", "0") };
        assert!(finds_element_by_id(&Compiler::default()));
    }
}
