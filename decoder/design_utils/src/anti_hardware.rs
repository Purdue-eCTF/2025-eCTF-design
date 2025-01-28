//! Utilities to defend against hardware glitching attacks

// this is pub use for proc macros to find it
pub use design_macros::{check_or_error_jump_table, create_mutations, rand_ops};
pub use rand_core::RngCore;

/// Performs the check multiple times if the check is true to protect against glitch attacks
#[macro_export]
macro_rules! multi_if {
    ($cond:expr, $true_action:expr, $false_action:expr, $rand:expr,) => {
        core::hint::black_box($crate::anti_hardware::rand_ops!($rand));

        if !core::hint::black_box($cond) {
            $false_action
        } else {
            core::hint::black_box($crate::anti_hardware::rand_ops!($rand));

            if core::hint::black_box($cond) {
                if core::hint::black_box($cond) {
                    core::hint::black_box($crate::anti_hardware::rand_ops!($rand));
                    if core::hint::black_box($cond) {
                        if core::hint::black_box($cond) {
                            core::hint::black_box($crate::anti_hardware::rand_ops!($rand));
                            if core::hint::black_box($cond) {
                                if core::hint::black_box($cond) {
                                    $true_action
                                } else {
                                    panic!("glitching detected");
                                }
                            } else {
                                panic!("glitching detected");
                            }
                        } else {
                            panic!("glitching detected");
                        }
                    } else {
                        panic!("glitching detected");
                    }
                } else {
                    panic!("glitching detected");
                }
            } else {
                panic!("glitching detected");
            }
        }
    };
}

#[macro_export]
macro_rules! const_time_equal_or_error {
    ($a:expr, $b:expr, $error:expr, $rand:expr,) => {
        $crate::multi_if!(
            $crate::subtle::ConstantTimeEq::ct_eq($a, $b).into(),
            (),
            { return Err($error) },
            $rand,
        );
    };
}

#[macro_export]
macro_rules! const_time_equal_or_error_jump_table {
    ($a:expr, $b:expr, $success_type:ty, $success_fn:tt, $args:tt, $error:expr, $rand:expr,) => {
        $crate::anti_hardware::check_or_error_jump_table!(
            $crate::subtle::ConstantTimeEq::ct_eq($a, $b).into(),
            $success_type,
            $success_fn,
            $args,
            $error,
            $rand,
        )
    };
}

#[doc(hidden)]
pub fn glitch_fail() {
    panic!("glitching detected");
}
