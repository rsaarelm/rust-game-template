//! Write macros that don't expect you to check their results.

#[macro_export]
macro_rules! write {
    ($dst:expr, $($arg:tt)*) => { std::write!($dst, $($arg)*).unwrap() };
}

#[macro_export]
macro_rules! writeln {
    ($dst:expr $(,)?) => { std::writeln!($dst).unwrap() };
    ($dst:expr, $($arg:tt)*) => { std::writeln!($dst, $($arg)*).unwrap() };
}
